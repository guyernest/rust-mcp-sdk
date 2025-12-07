use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Production default for GraphQL API
const DEFAULT_GRAPHQL_URL: &str = "https://api.pmcp.run/graphql";

// GraphQL URL - reads from environment variable or uses default
fn get_graphql_url() -> String {
    std::env::var("PMCP_RUN_GRAPHQL_URL").unwrap_or_else(|_| DEFAULT_GRAPHQL_URL.to_string())
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

/// Response from getUploadUrls mutation
#[derive(Debug, Deserialize)]
pub struct UploadUrls {
    #[serde(rename = "templateUploadUrl")]
    pub template_upload_url: String,
    #[serde(rename = "templateS3Key")]
    pub template_s3_key: String,
    #[serde(rename = "bootstrapUploadUrl")]
    pub bootstrap_upload_url: String,
    #[serde(rename = "bootstrapS3Key")]
    pub bootstrap_s3_key: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i32, // 900 seconds (15 minutes)
}

/// Response from createDeploymentFromS3 mutation
#[derive(Debug, Deserialize)]
pub struct DeploymentInfo {
    #[serde(rename = "deploymentId")]
    pub deployment_id: String,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Deployment status from getDeployment query
#[derive(Debug, Deserialize)]
pub struct DeploymentStatus {
    pub id: String,
    pub status: String,
    pub url: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
}

/// Get presigned S3 upload URLs
pub async fn get_upload_urls(
    access_token: &str,
    project_name: &str,
    template_size: usize,
    bootstrap_size: usize,
) -> Result<UploadUrls> {
    let query = r#"
        mutation GetUploadUrls(
            $projectName: String!,
            $templateSize: Int!,
            $bootstrapSize: Int!
        ) {
            getUploadUrls(
                projectName: $projectName,
                templateSize: $templateSize,
                bootstrapSize: $bootstrapSize
            ) {
                templateUploadUrl
                templateS3Key
                bootstrapUploadUrl
                bootstrapS3Key
                expiresIn
            }
        }
    "#;

    let variables = serde_json::json!({
        "projectName": project_name,
        "templateSize": template_size as i64,
        "bootstrapSize": bootstrap_size as i64
    });

    #[derive(Debug, Deserialize)]
    struct GetUploadUrlsResponse {
        #[serde(rename = "getUploadUrls")]
        get_upload_urls: UploadUrls,
    }

    let response: GetUploadUrlsResponse = execute_graphql(access_token, query, variables).await?;

    Ok(response.get_upload_urls)
}

/// Upload file directly to S3 using presigned URL
pub async fn upload_to_s3(url: &str, content: Vec<u8>, content_type: &str) -> Result<()> {
    let client = reqwest::Client::new();

    // Retry with exponential backoff for network failures
    for attempt in 1..=3 {
        let response = client
            .put(url)
            .header("Content-Type", content_type)
            .body(content.clone())
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            },
            Ok(resp) => {
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();

                if attempt < 3 {
                    eprintln!(
                        "⚠️  S3 upload failed (attempt {}/3): {} - {}. Retrying...",
                        attempt, status, error_text
                    );
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                } else {
                    bail!(
                        "S3 upload failed after 3 attempts: {} - {}",
                        status,
                        error_text
                    );
                }
            },
            Err(e) if attempt < 3 => {
                eprintln!(
                    "⚠️  S3 upload failed (attempt {}/3): {}. Retrying...",
                    attempt, e
                );
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            },
            Err(e) => {
                bail!("S3 upload failed after 3 attempts: {}", e);
            },
        }
    }

    Ok(())
}

/// Create deployment from S3 files
pub async fn create_deployment_from_s3(
    access_token: &str,
    urls: &UploadUrls,
    project_name: &str,
) -> Result<DeploymentInfo> {
    let query = r#"
        mutation CreateDeploymentFromS3(
            $templateS3Key: String!,
            $bootstrapS3Key: String!,
            $projectName: String!,
            $runtime: String,
            $memorySize: Int,
            $timeout: Int
        ) {
            createDeploymentFromS3(
                templateS3Key: $templateS3Key,
                bootstrapS3Key: $bootstrapS3Key,
                projectName: $projectName,
                runtime: $runtime,
                memorySize: $memorySize,
                timeout: $timeout
            ) {
                deploymentId
                status
                projectName
                createdAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "templateS3Key": urls.template_s3_key,
        "bootstrapS3Key": urls.bootstrap_s3_key,
        "projectName": project_name,
        "runtime": "provided.al2023",
        "memorySize": 512,
        "timeout": 30
    });

    #[derive(Debug, Deserialize)]
    struct CreateDeploymentResponse {
        #[serde(rename = "createDeploymentFromS3")]
        create_deployment_from_s3: Option<DeploymentInfo>,
    }

    let response: CreateDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .create_deployment_from_s3
        .context("Deployment creation returned null - check pmcp.run service logs")
}

/// Get deployment status
pub async fn get_deployment(access_token: &str, deployment_id: &str) -> Result<DeploymentStatus> {
    let query = r#"
        query GetDeployment($id: ID!) {
            getDeployment(id: $id) {
                id
                status
                url
                errorMessage
                createdAt
                completedAt
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": deployment_id
    });

    #[derive(Debug, Deserialize)]
    struct GetDeploymentResponse {
        #[serde(rename = "getDeployment")]
        get_deployment: Option<DeploymentStatus>,
    }

    let response: GetDeploymentResponse = execute_graphql(access_token, query, variables).await?;

    response.get_deployment.context("Deployment not found")
}

/// Execute GraphQL query
async fn execute_graphql<T>(
    access_token: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let client = reqwest::Client::new();
    let graphql_url = get_graphql_url();

    let request = GraphQLRequest {
        query: query.to_string(),
        variables,
    };

    let response = client
        .post(&graphql_url)
        .header("Authorization", access_token)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send GraphQL request")?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        bail!("GraphQL request failed: {}", error_text);
    }

    // Get raw text first for debugging
    let response_text = response.text().await.context("Failed to read response")?;

    // Try to parse as generic JSON first to check for errors
    let raw_json: serde_json::Value =
        serde_json::from_str(&response_text).context("Failed to parse response as JSON")?;

    // Check for GraphQL errors in raw response
    if let Some(errors) = raw_json.get("errors") {
        if let Some(errors_array) = errors.as_array() {
            let error_messages: Vec<String> = errors_array
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .map(|s| s.to_string())
                .collect();
            if !error_messages.is_empty() {
                bail!("GraphQL errors: {}", error_messages.join(", "));
            }
        }
    }

    // Now parse as the expected type
    let graphql_response: GraphQLResponse<T> = serde_json::from_str(&response_text)
        .with_context(|| format!("Failed to parse GraphQL response: {}", response_text))?;

    graphql_response
        .data
        .with_context(|| format!("No data in GraphQL response: {}", response_text))
}

/// Destroy deployment by ID (complete cleanup including CloudFormation stack)
///
/// This performs a complete cleanup:
/// - Deletes CloudFormation stack
/// - Removes OAuth configuration and Cognito User Pool
/// - Deletes McpServer registry entry
/// - Deletes Deployment DynamoDB record
pub async fn destroy_deployment(access_token: &str, deployment_id: &str) -> Result<()> {
    let query = r#"
        mutation DestroyDeployment($id: ID!) {
            destroyDeployment(id: $id) {
                id
                stackName
                status
                message
            }
        }
    "#;

    let variables = serde_json::json!({
        "id": deployment_id
    });

    #[derive(Debug, Deserialize)]
    struct DestroyDeploymentResponse {
        #[serde(rename = "destroyDeployment")]
        destroy_deployment: Option<DestroyResult>,
    }

    #[derive(Debug, Deserialize)]
    struct DestroyResult {
        id: String,
        #[serde(rename = "stackName")]
        stack_name: Option<String>,
        status: String,
        message: Option<String>,
    }

    let response: DestroyDeploymentResponse =
        execute_graphql(access_token, query, variables).await?;

    match response.destroy_deployment {
        Some(result) => {
            if result.status == "failed" {
                bail!(
                    "Failed to destroy deployment: {}",
                    result
                        .message
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
            }
            Ok(())
        },
        None => bail!("Failed to destroy deployment: no response returned"),
    }
}

/// Find deployment ID by project name
pub async fn find_deployment_id_by_name(access_token: &str, project_name: &str) -> Result<String> {
    let query = r#"
        query ListDeployments {
            listDeployments {
                items {
                    id
                    projectName
                }
            }
        }
    "#;

    let variables = serde_json::json!({});

    #[derive(Debug, Deserialize)]
    struct ListDeploymentsResponse {
        #[serde(rename = "listDeployments")]
        list_deployments: DeploymentList,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentList {
        items: Vec<DeploymentItem>,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentItem {
        id: String,
        #[serde(rename = "projectName")]
        project_name: String,
    }

    let response: ListDeploymentsResponse = execute_graphql(access_token, query, variables).await?;

    // Find deployment by project name
    response
        .list_deployments
        .items
        .iter()
        .find(|d| d.project_name == project_name)
        .map(|d| d.id.clone())
        .context(format!("No deployment found for project: {}", project_name))
}

/// Get deployment outputs (for outputs command)
pub async fn get_deployment_outputs(
    access_token: &str,
    project_name: &str,
) -> Result<crate::deployment::r#trait::DeploymentOutputs> {
    // Reuse get_deployment but find by project name
    let query = r#"
        query ListDeployments {
            listDeployments {
                items {
                    id
                    projectName
                    status
                    url
                }
            }
        }
    "#;

    let variables = serde_json::json!({});

    #[derive(Debug, Deserialize)]
    struct ListDeploymentsResponse {
        #[serde(rename = "listDeployments")]
        list_deployments: DeploymentList,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentList {
        items: Vec<DeploymentItem>,
    }

    #[derive(Debug, Deserialize)]
    struct DeploymentItem {
        id: String,
        #[serde(rename = "projectName")]
        project_name: String,
        status: String,
        url: Option<String>,
    }

    let response: ListDeploymentsResponse = execute_graphql(access_token, query, variables).await?;

    // Find deployment by project name
    let deployment = response
        .list_deployments
        .items
        .iter()
        .find(|d| d.project_name == project_name)
        .context(format!("No deployment found for project: {}", project_name))?;

    Ok(crate::deployment::r#trait::DeploymentOutputs {
        url: deployment.url.clone(),
        additional_urls: vec![],
        regions: vec![],
        stack_name: None,
        version: None,
        custom: std::collections::HashMap::new(),
    })
}

// ========== Landing Page Deployment GraphQL Functions ==========

/// Response from getLandingUploadUrl mutation
#[derive(Debug, Deserialize)]
pub struct LandingUploadUrl {
    #[serde(rename = "uploadUrl")]
    pub upload_url: String,
    #[serde(rename = "s3Key")]
    pub s3_key: String,
    #[serde(rename = "s3Bucket")]
    pub s3_bucket: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i32,
}

/// Response from deployLandingPage mutation
#[derive(Debug, Deserialize)]
pub struct LandingInfo {
    #[serde(rename = "landingId")]
    pub landing_id: String,
    #[serde(rename = "amplifyAppId")]
    pub amplify_app_id: String,
    #[serde(rename = "amplifyDomainUrl")]
    pub amplify_domain_url: String,
    pub status: String,
    #[serde(rename = "buildJobId")]
    pub build_job_id: String,
}

/// Landing page status from getLandingStatus mutation
#[derive(Debug, Deserialize)]
pub struct LandingStatus {
    pub id: String,
    #[serde(rename = "serverId")]
    pub server_id: String,
    pub status: String,
    #[serde(rename = "amplifyDomainUrl")]
    pub amplify_domain_url: Option<String>,
    #[serde(rename = "customDomain")]
    pub custom_domain: Option<String>,
    #[serde(rename = "lastDeployedAt")]
    pub last_deployed_at: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

/// Get presigned S3 upload URL for landing page zip
pub async fn get_landing_upload_url(
    access_token: &str,
    server_id: &str,
    zip_size: usize,
) -> Result<LandingUploadUrl> {
    let query = r#"
        mutation GetLandingUploadUrl(
            $serverId: String!,
            $fileSize: Int!
        ) {
            getLandingUploadUrl(
                serverId: $serverId,
                fileSize: $fileSize
            ) {
                uploadUrl
                s3Key
                s3Bucket
                expiresIn
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "fileSize": zip_size as i64
    });

    #[derive(Debug, Deserialize)]
    struct GetLandingUploadUrlResponse {
        #[serde(rename = "getLandingUploadUrl")]
        get_landing_upload_url: LandingUploadUrl,
    }

    let response: GetLandingUploadUrlResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.get_landing_upload_url)
}

/// Deploy landing page from S3 zip file
pub async fn deploy_landing_page(
    access_token: &str,
    s3_key: &str,
    server_id: &str,
    server_name: &str,
    config_json: &str,
) -> Result<LandingInfo> {
    let query = r#"
        mutation DeployLandingPage(
            $serverId: String!,
            $serverName: String!,
            $sourceS3Key: String!,
            $config: AWSJSON!
        ) {
            deployLandingPage(
                serverId: $serverId,
                serverName: $serverName,
                sourceS3Key: $sourceS3Key,
                config: $config
            ) {
                landingId
                amplifyAppId
                amplifyDomainUrl
                status
                buildJobId
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "serverName": server_name,
        "sourceS3Key": s3_key,
        "config": config_json
    });

    #[derive(Debug, Deserialize)]
    struct DeployLandingResponse {
        #[serde(rename = "deployLandingPage")]
        deploy_landing_page: LandingInfo,
    }

    let response: DeployLandingResponse = execute_graphql(access_token, query, variables).await?;

    Ok(response.deploy_landing_page)
}

/// Get landing page status
/// NOTE: This is a MUTATION, not a Query! It checks Amplify job status and updates DB.
pub async fn get_landing_status(access_token: &str, landing_id: &str) -> Result<LandingStatus> {
    let query = r#"
        mutation GetLandingStatus($landingId: String!) {
            getLandingStatus(landingId: $landingId) {
                id
                serverId
                status
                amplifyDomainUrl
                customDomain
                lastDeployedAt
                errorMessage
            }
        }
    "#;

    let variables = serde_json::json!({
        "landingId": landing_id
    });

    #[derive(Debug, Deserialize)]
    struct GetLandingStatusResponse {
        #[serde(rename = "getLandingStatus")]
        get_landing_status: Option<LandingStatus>,
    }

    let response: GetLandingStatusResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .get_landing_status
        .context("Landing page not found")
}

// ========== OAuth Configuration GraphQL Functions ==========

/// OAuth configuration response from configureServerOAuth mutation
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    #[serde(rename = "serverId")]
    pub server_id: String,
    #[serde(rename = "oauthEnabled")]
    pub oauth_enabled: bool,
    #[serde(rename = "userPoolId")]
    pub user_pool_id: Option<String>,
    #[serde(rename = "userPoolRegion")]
    pub user_pool_region: Option<String>,
    #[serde(rename = "discoveryUrl")]
    pub discovery_url: Option<String>,
    #[serde(rename = "registrationEndpoint")]
    pub registration_endpoint: Option<String>,
    #[serde(rename = "authorizationEndpoint")]
    pub authorization_endpoint: Option<String>,
    #[serde(rename = "tokenEndpoint")]
    pub token_endpoint: Option<String>,
}

/// OAuth endpoints response from fetchServerOAuthEndpoints query
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthEndpoints {
    #[serde(rename = "serverId")]
    pub server_id: String,
    #[serde(rename = "oauthEnabled")]
    pub oauth_enabled: bool,
    pub provider: Option<String>,
    #[serde(rename = "userPoolId")]
    pub user_pool_id: Option<String>,
    #[serde(rename = "userPoolRegion")]
    pub user_pool_region: Option<String>,
    pub scopes: Option<Vec<String>>,
    #[serde(rename = "dcrEnabled")]
    pub dcr_enabled: Option<bool>,
    #[serde(rename = "discoveryUrl")]
    pub discovery_url: Option<String>,
    #[serde(rename = "registrationEndpoint")]
    pub registration_endpoint: Option<String>,
    #[serde(rename = "authorizationEndpoint")]
    pub authorization_endpoint: Option<String>,
    #[serde(rename = "tokenEndpoint")]
    pub token_endpoint: Option<String>,
}

/// Configure OAuth for an MCP server
///
/// This creates a Cognito User Pool if one doesn't exist and configures
/// the API Gateway routes with the shared authorizer Lambda.
pub async fn configure_server_oauth(
    access_token: &str,
    server_id: &str,
    enabled: bool,
    scopes: Option<Vec<String>>,
    dcr_enabled: Option<bool>,
    public_client_patterns: Option<Vec<String>>,
    shared_pool_name: Option<String>,
) -> Result<OAuthConfig> {
    let query = r#"
        mutation ConfigureServerOAuth(
            $serverId: String!
            $enabled: Boolean!
            $scopes: [String]
            $dcrEnabled: Boolean
            $publicClientPatterns: [String]
            $sharedPoolName: String
        ) {
            configureServerOAuth(
                serverId: $serverId
                enabled: $enabled
                scopes: $scopes
                dcrEnabled: $dcrEnabled
                publicClientPatterns: $publicClientPatterns
                sharedPoolName: $sharedPoolName
            ) {
                serverId
                oauthEnabled
                userPoolId
                userPoolRegion
                discoveryUrl
                registrationEndpoint
                authorizationEndpoint
                tokenEndpoint
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id,
        "enabled": enabled,
        "scopes": scopes,
        "dcrEnabled": dcr_enabled,
        "publicClientPatterns": public_client_patterns,
        "sharedPoolName": shared_pool_name
    });

    #[derive(Debug, Deserialize)]
    struct ConfigureServerOAuthResponse {
        #[serde(rename = "configureServerOAuth")]
        configure_server_oauth: OAuthConfig,
    }

    let response: ConfigureServerOAuthResponse =
        execute_graphql(access_token, query, variables).await?;

    Ok(response.configure_server_oauth)
}

/// Disable OAuth for an MCP server
pub async fn disable_server_oauth(access_token: &str, server_id: &str) -> Result<()> {
    let query = r#"
        mutation DisableServerOAuth($serverId: String!) {
            disableServerOAuth(serverId: $serverId) {
                serverId
                oauthEnabled
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id
    });

    #[derive(Debug, Deserialize)]
    struct DisableServerOAuthResponse {
        #[serde(rename = "disableServerOAuth")]
        disable_server_oauth: DisableResult,
    }

    #[derive(Debug, Deserialize)]
    struct DisableResult {
        #[serde(rename = "serverId")]
        _server_id: String,
        #[serde(rename = "oauthEnabled")]
        oauth_enabled: bool,
    }

    let response: DisableServerOAuthResponse =
        execute_graphql(access_token, query, variables).await?;

    if response.disable_server_oauth.oauth_enabled {
        bail!("Failed to disable OAuth - server still reports OAuth enabled");
    }

    Ok(())
}

/// Fetch OAuth endpoints for an MCP server
pub async fn fetch_server_oauth_endpoints(
    access_token: &str,
    server_id: &str,
) -> Result<OAuthEndpoints> {
    let query = r#"
        query FetchServerOAuthEndpoints($serverId: String!) {
            fetchServerOAuthEndpoints(serverId: $serverId) {
                serverId
                oauthEnabled
                provider
                userPoolId
                userPoolRegion
                scopes
                dcrEnabled
                discoveryUrl
                registrationEndpoint
                authorizationEndpoint
                tokenEndpoint
            }
        }
    "#;

    let variables = serde_json::json!({
        "serverId": server_id
    });

    #[derive(Debug, Deserialize)]
    struct FetchServerOAuthEndpointsResponse {
        #[serde(rename = "fetchServerOAuthEndpoints")]
        fetch_server_oauth_endpoints: Option<OAuthEndpoints>,
    }

    let response: FetchServerOAuthEndpointsResponse =
        execute_graphql(access_token, query, variables).await?;

    response
        .fetch_server_oauth_endpoints
        .context("OAuth not configured for this server")
}
