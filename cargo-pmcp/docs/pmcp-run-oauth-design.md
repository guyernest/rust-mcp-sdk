# pmcp.run Multi-Tenant OAuth Architecture

## Overview

This document describes the OAuth architecture for pmcp.run using AWS Lambda's new tenant isolation mode (announced November 2025). This approach enables shared OAuth infrastructure across all MCP servers while maintaining security isolation per server.

## Goals

1. **Simplify deployment**: Users only upload the MCP Server Lambda
2. **Cost efficiency**: Share OAuth Lambdas across all pmcp.run servers
3. **Security isolation**: Leverage Lambda tenant isolation for per-server data separation
4. **Flexibility**: Support both per-server and shared Cognito User Pools

## Architecture

### Current State (without OAuth)

```
┌─────────────────────────────────────────────────────────────────┐
│  pmcp.run Infrastructure                                        │
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │ Server A    │    │ Server B    │    │ Server C    │         │
│  │ Lambda      │    │ Lambda      │    │ Lambda      │         │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘         │
│         │                  │                  │                 │
│         └──────────────────┼──────────────────┘                 │
│                            │                                    │
│                   ┌────────▼────────┐                           │
│                   │  API Gateway    │                           │
│                   │  (per server)   │                           │
│                   └─────────────────┘                           │
└─────────────────────────────────────────────────────────────────┘
```

### Proposed State (with Multi-Tenant OAuth)

```
┌─────────────────────────────────────────────────────────────────┐
│  pmcp.run Infrastructure                                        │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Shared OAuth Infrastructure                             │   │
│  │                                                          │   │
│  │  ┌────────────────────┐    ┌────────────────────┐       │   │
│  │  │ OAuth Proxy Lambda │    │ Authorizer Lambda  │       │   │
│  │  │ (PER_TENANT mode)  │    │ (PER_TENANT mode)  │       │   │
│  │  │                    │    │                    │       │   │
│  │  │ Tenants:           │    │ Tenants:           │       │   │
│  │  │ - server-a         │    │ - server-a         │       │   │
│  │  │ - server-b         │    │ - server-b         │       │   │
│  │  │ - server-c         │    │ - server-c         │       │   │
│  │  └────────────────────┘    └────────────────────┘       │   │
│  │                                                          │   │
│  │  ┌────────────────────────────────────────────┐         │   │
│  │  │ ServerOAuthConfig (DynamoDB)               │         │   │
│  │  │ ClientRegistration (DynamoDB)              │         │   │
│  │  └────────────────────────────────────────────┘         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │ Server A    │    │ Server B    │    │ Server C    │         │
│  │ Lambda      │    │ Lambda      │    │ Lambda      │         │
│  └─────────────┘    └─────────────┘    └─────────────┘         │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  API Gateway (shared, tenant-aware routing)              │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Lambda Tenant Isolation Mode

AWS Lambda's tenant isolation mode (November 2025) provides:

- **Per-tenant execution environments**: Each tenant gets isolated compute
- **Memory/disk isolation**: Data cached in memory or `/tmp` is tenant-specific
- **Warm start benefits**: Execution environments are reused within a tenant
- **Automatic routing**: Lambda routes requests based on `X-Amz-Tenant-Id` header

### Creating Tenant-Isolated Lambdas

```bash
# OAuth Proxy Lambda
aws lambda create-function \
  --function-name pmcp-run-oauth-proxy \
  --runtime provided.al2023 \
  --handler bootstrap \
  --zip-file fileb://oauth-proxy.zip \
  --role arn:aws:iam::ACCOUNT:role/pmcp-run-oauth-role \
  --tenancy-config '{"TenantIsolationMode": "PER_TENANT"}'

# Authorizer Lambda
aws lambda create-function \
  --function-name pmcp-run-authorizer \
  --runtime provided.al2023 \
  --handler bootstrap \
  --zip-file fileb://authorizer.zip \
  --role arn:aws:iam::ACCOUNT:role/pmcp-run-authorizer-role \
  --tenancy-config '{"TenantIsolationMode": "PER_TENANT"}'
```

### Invoking with Tenant ID

```bash
# Via AWS CLI
aws lambda invoke \
  --function-name pmcp-run-oauth-proxy \
  --tenant-id chess-server-abc123 \
  response.json

# Via API Gateway (automatic header injection)
# X-Amz-Tenant-Id: chess-server-abc123
```

## Data Model

### ServerOAuthConfig Table

Stores OAuth configuration per MCP server.

```
Table: pmcp-run-server-oauth-config
Partition Key: server_id (String)

Attributes:
┌─────────────────────┬─────────────────────────────────────────┐
│ server_id           │ "chess-abc123" (PK)                     │
│ user_pool_id        │ "eu-west-1_XyZ123"                      │
│ user_pool_region    │ "eu-west-1"                             │
│ oauth_enabled       │ true                                    │
│ provider            │ "cognito"                               │
│ scopes              │ ["openid", "email", "mcp/read"]         │
│ dcr_enabled         │ true                                    │
│ public_client_patterns │ ["claude", "cursor", "chatgpt"]      │
│ shared_pool_name    │ null (or "org-main" if shared)          │
│ created_at          │ 1701619200                              │
│ updated_at          │ 1701619200                              │
└─────────────────────┴─────────────────────────────────────────┘
```

### ClientRegistration Table

Stores dynamically registered OAuth clients.

```
Table: pmcp-run-client-registration
Partition Key: client_id (String)
GSI: server_id-index (server_id -> client_id)

Attributes:
┌─────────────────────┬─────────────────────────────────────────┐
│ client_id           │ "uuid-xxxx-xxxx" (PK)                   │
│ server_id           │ "chess-abc123" (GSI)                    │
│ client_name         │ "claude-desktop"                        │
│ client_secret_hash  │ "sha256:xxxx" (null if public)          │
│ redirect_uris       │ ["http://localhost:8080/callback"]      │
│ grant_types         │ ["authorization_code", "refresh_token"] │
│ response_types      │ ["code"]                                │
│ scope               │ "openid email mcp/read"                 │
│ is_public           │ true                                    │
│ created_at          │ 1701619200                              │
└─────────────────────┴─────────────────────────────────────────┘
```

## API Gateway Configuration

### Route Structure

All routes include the server ID for tenant routing:

```
Base URL: https://api.pmcp.run

OAuth Routes (public, no auth required):
  GET  /{serverId}/.well-known/openid-configuration
  GET  /{serverId}/.well-known/oauth-authorization-server
  POST /{serverId}/oauth2/register
  GET  /{serverId}/oauth2/authorize
  POST /{serverId}/oauth2/token
  POST /{serverId}/oauth2/revoke

MCP Routes (protected, requires valid token):
  POST /{serverId}/mcp
  POST /{serverId}/mcp/{proxy+}

Health Check (public):
  GET  /{serverId}/health
```

### Tenant ID Injection (CDK)

```typescript
import * as apigateway from 'aws-cdk-lib/aws-apigatewayv2';
import * as lambda from 'aws-cdk-lib/aws-lambda';

// Shared OAuth Proxy Lambda
const oauthProxyLambda = lambda.Function.fromFunctionName(
  this, 'OAuthProxy', 'pmcp-run-oauth-proxy'
);

// Integration with tenant ID mapping
const oauthIntegration = new apigateway.HttpLambdaIntegration(
  'OAuthIntegration',
  oauthProxyLambda,
  {
    parameterMapping: new apigateway.ParameterMapping()
      .custom('X-Amz-Tenant-Id', '$request.path.serverId'),
  }
);

// OAuth discovery route
httpApi.addRoutes({
  path: '/{serverId}/.well-known/openid-configuration',
  methods: [apigateway.HttpMethod.GET],
  integration: oauthIntegration,
});

// OAuth register route
httpApi.addRoutes({
  path: '/{serverId}/oauth2/register',
  methods: [apigateway.HttpMethod.POST],
  integration: oauthIntegration,
});

// ... other OAuth routes
```

### Authorizer Configuration

```typescript
import * as authorizers from 'aws-cdk-lib/aws-apigatewayv2-authorizers';

// Shared Authorizer Lambda
const authorizerLambda = lambda.Function.fromFunctionName(
  this, 'Authorizer', 'pmcp-run-authorizer'
);

// Lambda authorizer with tenant ID
const authorizer = new authorizers.HttpLambdaAuthorizer(
  'TenantAuthorizer',
  authorizerLambda,
  {
    authorizerName: 'pmcp-run-tenant-authorizer',
    identitySource: [
      '$request.header.Authorization',
      '$request.path.serverId',  // Used for tenant routing
    ],
    responseTypes: [authorizers.HttpLambdaResponseType.SIMPLE],
    resultsCacheTtl: cdk.Duration.seconds(300),
  }
);

// MCP route with authorizer
httpApi.addRoutes({
  path: '/{serverId}/mcp',
  methods: [apigateway.HttpMethod.POST],
  integration: mcpIntegration,
  authorizer: authorizer,
});
```

## OAuth Proxy Lambda Implementation

### Request Handler (Rust)

```rust
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use aws_sdk_dynamodb::Client as DynamoClient;

// Tenant config is cached in memory (isolated per tenant by Lambda)
static TENANT_CONFIG: OnceLock<ServerOAuthConfig> = OnceLock::new();

async fn handler(event: Request, context: lambda_runtime::Context) -> Result<Response<Body>, Error> {
    // Get tenant ID from Lambda context (injected by Lambda runtime)
    let tenant_id = context.tenant_id
        .ok_or_else(|| Error::from("Missing tenant ID"))?;

    let path = event.uri().path();

    // Load tenant config (cached in tenant-isolated memory)
    let config = get_tenant_config(&tenant_id).await?;

    match path {
        p if p.ends_with("/.well-known/openid-configuration") => {
            handle_oidc_discovery(&config).await
        }
        p if p.ends_with("/oauth2/register") => {
            handle_client_registration(&config, &tenant_id, event).await
        }
        p if p.ends_with("/oauth2/authorize") => {
            handle_authorize(&config, event).await
        }
        p if p.ends_with("/oauth2/token") => {
            handle_token(&config, event).await
        }
        _ => Ok(Response::builder().status(404).body(Body::Empty)?)
    }
}

async fn get_tenant_config(tenant_id: &str) -> Result<&'static ServerOAuthConfig, Error> {
    // Check cache first (isolated per tenant!)
    if let Some(config) = TENANT_CONFIG.get() {
        return Ok(config);
    }

    // Load from DynamoDB
    let dynamodb = get_dynamodb_client().await;
    let result = dynamodb
        .get_item()
        .table_name("pmcp-run-server-oauth-config")
        .key("server_id", AttributeValue::S(tenant_id.to_string()))
        .send()
        .await?;

    let config = parse_config(result.item())?;
    let _ = TENANT_CONFIG.set(config);

    Ok(TENANT_CONFIG.get().unwrap())
}
```

### OIDC Discovery Response

```rust
async fn handle_oidc_discovery(config: &ServerOAuthConfig) -> Result<Response<Body>, Error> {
    let base_url = format!("https://api.pmcp.run/{}", config.server_id);
    let cognito_issuer = format!(
        "https://cognito-idp.{}.amazonaws.com/{}",
        config.user_pool_region, config.user_pool_id
    );

    let discovery = serde_json::json!({
        "issuer": cognito_issuer,
        "authorization_endpoint": format!("{}/oauth2/authorize", base_url),
        "token_endpoint": format!("{}/oauth2/token", base_url),
        "registration_endpoint": format!("{}/oauth2/register", base_url),
        "jwks_uri": format!("{}/.well-known/jwks.json", cognito_issuer),
        "revocation_endpoint": format!("{}/oauth2/revoke", base_url),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": config.scopes,
    });

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&discovery)?))?)
}
```

## Authorizer Lambda Implementation

### JWT Validation with Tenant Isolation

```rust
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::collections::HashMap;
use std::sync::OnceLock;

// JWKS cache - isolated per tenant by Lambda!
static JWKS_CACHE: OnceLock<HashMap<String, DecodingKey>> = OnceLock::new();

async fn handler(event: AuthorizerRequest, context: lambda_runtime::Context) -> Result<AuthorizerResponse, Error> {
    let tenant_id = context.tenant_id
        .ok_or_else(|| Error::from("Missing tenant ID"))?;

    // Extract token
    let token = extract_bearer_token(&event)?;

    // Load tenant config (cached per tenant)
    let config = get_tenant_config(&tenant_id).await?;

    // Validate token (JWKS cached per tenant!)
    match validate_token(&config, &token).await {
        Ok(claims) => {
            // Return allow policy with claims in context
            Ok(AuthorizerResponse {
                is_authorized: true,
                context: HashMap::from([
                    ("sub".to_string(), claims.sub),
                    ("scope".to_string(), claims.scope),
                    ("tenantId".to_string(), tenant_id),
                ]),
            })
        }
        Err(e) => {
            tracing::warn!("Token validation failed for tenant {}: {}", tenant_id, e);
            Ok(AuthorizerResponse {
                is_authorized: false,
                context: HashMap::new(),
            })
        }
    }
}

async fn validate_token(config: &ServerOAuthConfig, token: &str) -> Result<Claims, Error> {
    // Get or fetch JWKS (cached in tenant-isolated memory)
    let jwks = get_jwks(config).await?;

    // Decode and validate
    let header = jsonwebtoken::decode_header(token)?;
    let kid = header.kid.ok_or("No kid in token")?;
    let key = jwks.get(&kid).ok_or("Unknown key ID")?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[format!(
        "https://cognito-idp.{}.amazonaws.com/{}",
        config.user_pool_region, config.user_pool_id
    )]);

    let token_data = decode::<Claims>(token, key, &validation)?;
    Ok(token_data.claims)
}
```

## Cognito User Pool Management

### Per-Server Mode (Default)

Each MCP server gets its own Cognito User Pool:

```rust
async fn create_user_pool_for_server(server_id: &str, region: &str) -> Result<String, Error> {
    let cognito = get_cognito_client(region).await;

    let result = cognito
        .create_user_pool()
        .pool_name(format!("pmcp-{}", server_id))
        .auto_verified_attributes(VerifiedAttributeType::Email)
        .username_attributes(UsernameAttributeType::Email)
        .policies(UserPoolPolicyType::builder()
            .password_policy(PasswordPolicyType::builder()
                .minimum_length(8)
                .require_lowercase(true)
                .require_numbers(true)
                .build())
            .build())
        .send()
        .await?;

    let user_pool_id = result.user_pool().unwrap().id().unwrap();

    // Create resource server for MCP scopes
    cognito
        .create_resource_server()
        .user_pool_id(user_pool_id)
        .identifier("mcp")
        .name("MCP Server")
        .scopes(
            ResourceServerScopeType::builder()
                .scope_name("read")
                .scope_description("Read access to MCP tools")
                .build(),
        )
        .scopes(
            ResourceServerScopeType::builder()
                .scope_name("write")
                .scope_description("Write access to MCP tools")
                .build(),
        )
        .send()
        .await?;

    Ok(user_pool_id.to_string())
}
```

### Shared Mode (Opt-in)

Multiple servers share a User Pool:

```rust
async fn get_or_create_shared_pool(shared_name: &str, region: &str) -> Result<String, Error> {
    let dynamodb = get_dynamodb_client().await;

    // Check if shared pool exists
    let result = dynamodb
        .get_item()
        .table_name("pmcp-run-shared-pools")
        .key("pool_name", AttributeValue::S(shared_name.to_string()))
        .send()
        .await?;

    if let Some(item) = result.item() {
        return Ok(item.get("user_pool_id").unwrap().as_s().unwrap().to_string());
    }

    // Create new shared pool
    let user_pool_id = create_shared_user_pool(shared_name, region).await?;

    // Store reference
    dynamodb
        .put_item()
        .table_name("pmcp-run-shared-pools")
        .item("pool_name", AttributeValue::S(shared_name.to_string()))
        .item("user_pool_id", AttributeValue::S(user_pool_id.clone()))
        .item("region", AttributeValue::S(region.to_string()))
        .send()
        .await?;

    Ok(user_pool_id)
}
```

## GraphQL API Extensions

### Schema Additions

```graphql
input OAuthConfigInput {
  enabled: Boolean!
  provider: String!  # "cognito"
  sharedPoolName: String  # null for per-server, or "org-main" for shared
  scopes: [String!]
  dcrEnabled: Boolean
  publicClientPatterns: [String!]
}

type OAuthEndpoints {
  discoveryUrl: String!
  registrationUrl: String!
  authorizeUrl: String!
  tokenUrl: String!
  userPoolId: String!
  userPoolRegion: String!
}

type DeploymentWithOAuth {
  deploymentId: String!
  status: String!
  url: String
  oauth: OAuthEndpoints
}

type Mutation {
  # Extended deployment mutation with OAuth support
  createDeploymentFromS3WithOAuth(
    templateS3Key: String!
    bootstrapS3Key: String!
    serverName: String!
    oauthConfig: OAuthConfigInput
  ): DeploymentWithOAuth!

  # Standalone OAuth configuration
  configureOAuth(
    serverId: String!
    oauthConfig: OAuthConfigInput!
  ): OAuthEndpoints!

  # Disable OAuth for a server
  disableOAuth(serverId: String!): Boolean!
}

type Query {
  # Get OAuth configuration for a server
  getOAuthConfig(serverId: String!): OAuthEndpoints

  # List registered clients for a server
  listOAuthClients(serverId: String!): [OAuthClient!]!
}

type OAuthClient {
  clientId: String!
  clientName: String!
  isPublic: Boolean!
  createdAt: String!
}
```

### Resolver Implementation

```typescript
// createDeploymentFromS3WithOAuth resolver
export async function createDeploymentFromS3WithOAuth(
  _: any,
  args: {
    templateS3Key: string;
    bootstrapS3Key: string;
    serverName: string;
    oauthConfig?: OAuthConfigInput;
  },
  context: Context
): Promise<DeploymentWithOAuth> {
  const { templateS3Key, bootstrapS3Key, serverName, oauthConfig } = args;
  const userId = context.identity.sub;

  // Generate server ID
  const serverId = `${serverName}-${generateShortId()}`;

  let oauthEndpoints: OAuthEndpoints | undefined;

  if (oauthConfig?.enabled) {
    // Create or get Cognito User Pool
    const userPoolId = oauthConfig.sharedPoolName
      ? await getOrCreateSharedPool(oauthConfig.sharedPoolName)
      : await createUserPoolForServer(serverId);

    // Store OAuth config
    await dynamodb.put({
      TableName: 'pmcp-run-server-oauth-config',
      Item: {
        server_id: serverId,
        user_pool_id: userPoolId,
        user_pool_region: process.env.AWS_REGION,
        oauth_enabled: true,
        provider: oauthConfig.provider,
        scopes: oauthConfig.scopes || ['openid', 'email', 'mcp/read'],
        dcr_enabled: oauthConfig.dcrEnabled ?? true,
        public_client_patterns: oauthConfig.publicClientPatterns ||
          ['claude', 'cursor', 'chatgpt', 'mcp-inspector'],
        shared_pool_name: oauthConfig.sharedPoolName || null,
        created_at: Date.now(),
      },
    }).promise();

    // Build OAuth endpoints
    const baseUrl = `https://api.pmcp.run/${serverId}`;
    oauthEndpoints = {
      discoveryUrl: `${baseUrl}/.well-known/openid-configuration`,
      registrationUrl: `${baseUrl}/oauth2/register`,
      authorizeUrl: `${baseUrl}/oauth2/authorize`,
      tokenUrl: `${baseUrl}/oauth2/token`,
      userPoolId,
      userPoolRegion: process.env.AWS_REGION!,
    };
  }

  // Create deployment (existing logic)
  const deployment = await createDeployment({
    userId,
    serverId,
    serverName,
    templateS3Key,
    bootstrapS3Key,
    oauthEnabled: oauthConfig?.enabled ?? false,
  });

  return {
    deploymentId: deployment.id,
    status: deployment.status,
    url: deployment.url,
    oauth: oauthEndpoints,
  };
}
```

## Deployment Flow

### From cargo-pmcp Perspective

```bash
# 1. Initialize with OAuth
cargo pmcp deploy init --oauth cognito --target pmcp-run

# 2. Deploy (only uploads MCP Server Lambda)
cargo pmcp deploy --target pmcp-run

# Output:
# 🎉 Deployment successful!
#
# 📊 Deployment Details:
#    Name: chess
#    ID: chess-abc123
#    URL: https://api.pmcp.run/chess-abc123/mcp
#
# 🔐 OAuth Endpoints:
#    Discovery: https://api.pmcp.run/chess-abc123/.well-known/openid-configuration
#    Register:  https://api.pmcp.run/chess-abc123/oauth2/register
#    Authorize: https://api.pmcp.run/chess-abc123/oauth2/authorize
#    Token:     https://api.pmcp.run/chess-abc123/oauth2/token
#
#    User Pool ID: eu-west-1_XyZ123
```

### Sequence Diagram

```
┌─────────┐     ┌───────────┐     ┌──────────┐     ┌─────────┐
│cargo-   │     │pmcp.run   │     │Cognito   │     │DynamoDB │
│pmcp     │     │Backend    │     │          │     │         │
└────┬────┘     └─────┬─────┘     └────┬─────┘     └────┬────┘
     │                │                │                │
     │ deploy init    │                │                │
     │ --oauth cognito│                │                │
     │────────────────>                │                │
     │                │                │                │
     │ Upload Lambda  │                │                │
     │ + OAuth config │                │                │
     │────────────────>                │                │
     │                │                │                │
     │                │ Create User    │                │
     │                │ Pool           │                │
     │                │───────────────>│                │
     │                │                │                │
     │                │ user_pool_id   │                │
     │                │<───────────────│                │
     │                │                │                │
     │                │ Store OAuth    │                │
     │                │ config         │                │
     │                │───────────────────────────────>│
     │                │                │                │
     │                │ Deploy MCP     │                │
     │                │ Lambda         │                │
     │                │ (existing flow)│                │
     │                │                │                │
     │ Deployment +   │                │                │
     │ OAuth endpoints│                │                │
     │<────────────────                │                │
     │                │                │                │
```

## Security Considerations

### Tenant Isolation

1. **Execution Environment Isolation**: Lambda's `PER_TENANT` mode ensures each server's invocations run in isolated execution environments
2. **Memory Isolation**: JWKS cache, tenant config, and any in-memory data is isolated per tenant
3. **Disk Isolation**: Any files written to `/tmp` are isolated per tenant

### Cognito Security

1. **User Pool Isolation**: Per-server mode provides complete user isolation
2. **Scope Enforcement**: MCP scopes (`mcp/read`, `mcp/write`) are enforced by the authorizer
3. **Token Validation**: JWTs are validated against the correct User Pool for each tenant

### API Gateway Security

1. **Authorization**: All MCP routes require valid JWT tokens
2. **Rate Limiting**: Apply per-tenant rate limits to prevent abuse
3. **CORS**: Configure appropriate CORS headers per server

## Monitoring and Observability

### CloudWatch Logs

With JSON logging enabled, Lambda includes `tenantId` in all logs:

```json
{
  "timestamp": "2025-12-03T12:00:00.000Z",
  "level": "INFO",
  "message": "Processing OAuth request",
  "tenantId": "chess-abc123",
  "requestId": "xxx-xxx"
}
```

### CloudWatch Queries

```sql
-- Find logs for a specific tenant
fields @message
| filter tenantId='chess-abc123' or record.tenantId='chess-abc123'
| limit 1000

-- Count requests per tenant
fields tenantId
| filter tenantId != ''
| stats count() by tenantId
| sort count desc
```

### Metrics

Emit custom metrics per tenant:

```rust
cloudwatch.put_metric_data()
    .namespace("pmcp-run/oauth")
    .metric_data(
        MetricDatum::builder()
            .metric_name("TokenValidations")
            .value(1.0)
            .dimensions(
                Dimension::builder()
                    .name("TenantId")
                    .value(&tenant_id)
                    .build()
            )
            .build()
    )
    .send()
    .await?;
```

## Implementation Checklist

### Phase 1: Infrastructure Setup

- [ ] Create `pmcp-run-server-oauth-config` DynamoDB table
- [ ] Create `pmcp-run-client-registration` DynamoDB table
- [ ] Create `pmcp-run-shared-pools` DynamoDB table (for shared Cognito)
- [ ] Create IAM roles for OAuth Lambdas

### Phase 2: Shared Lambda Deployment

- [ ] Build OAuth Proxy Lambda (Rust)
- [ ] Build Authorizer Lambda (Rust)
- [ ] Deploy with `PER_TENANT` tenancy config
- [ ] Test tenant isolation

### Phase 3: API Gateway Integration

- [ ] Update API Gateway routes for OAuth endpoints
- [ ] Configure tenant ID injection from path parameter
- [ ] Configure Lambda authorizer with tenant support
- [ ] Test end-to-end OAuth flow

### Phase 4: GraphQL API

- [ ] Add `OAuthConfigInput` and related types
- [ ] Implement `createDeploymentFromS3WithOAuth` resolver
- [ ] Implement `configureOAuth` resolver
- [ ] Implement `getOAuthConfig` query
- [ ] Implement `listOAuthClients` query

### Phase 5: Cognito Management

- [ ] Implement per-server User Pool creation
- [ ] Implement shared User Pool management
- [ ] Implement resource server creation (MCP scopes)
- [ ] Test User Pool lifecycle

### Phase 6: cargo-pmcp Integration

- [ ] Update GraphQL client to send OAuth config
- [ ] Update deployment outputs to show OAuth endpoints
- [ ] Update documentation

## References

- [AWS Lambda Tenant Isolation Mode](https://aws.amazon.com/blogs/compute/building-multi-tenant-saas-applications-with-aws-lambdas-new-tenant-isolation-mode/)
- [MCP OAuth 2.1 Specification](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/authentication/)
- [RFC 7591 - Dynamic Client Registration](https://datatracker.ietf.org/doc/html/rfc7591)
- [AWS Cognito Developer Guide](https://docs.aws.amazon.com/cognito/latest/developerguide/)
