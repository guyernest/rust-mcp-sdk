# AWS Cognito

AWS Cognito is Amazon's identity service, providing user pools for authentication and identity pools for AWS resource access. This chapter covers Cognito integration for MCP servers.

> **Note:** Cognito is shown here as an example. If your organization already uses a different identity provider (Okta, Auth0, Entra ID, etc.), use that instead. The patterns in this chapter apply to any OIDC-compliant provider.

## The Easy Way: `cargo pmcp` + CDK

**The fastest path to production:** Use `cargo pmcp` to configure OAuth, then let the generated CDK handle Cognito setup. You don't need to manually create user pools, configure clients, or set up resource servers—the CDK does it all.

### Step 1: Initialize OAuth Configuration

```bash
# Initialize deployment with Cognito OAuth
cargo pmcp deploy init --target pmcp-run --oauth cognito

# This creates/updates .pmcp/deploy.toml with:
```

```toml
# .pmcp/deploy.toml
[auth]
enabled = true
provider = "cognito"
callback_urls = [
    "http://localhost:3000/callback",  # For development
]

[auth.dcr]
# Dynamic Client Registration for MCP clients
enabled = true
public_client_patterns = [
    "claude",
    "cursor",
    "chatgpt",
    "mcp-inspector",
]
default_scopes = [
    "openid",
    "email",
    "mcp/read",
]
allowed_scopes = [
    "openid",
    "email",
    "profile",
    "mcp/read",
    "mcp/write",
    "mcp/admin",
]
```

### Step 2: Deploy with CDK

The deployment generates a CDK stack that creates all Cognito resources:

```bash
# Build and deploy
cargo pmcp deploy

# The CDK stack creates:
# - Cognito User Pool with password policies
# - App client with OAuth flows configured
# - Resource server with MCP scopes
# - Optional: Federation with corporate IdP
```

### What the CDK Creates

The generated CDK stack (in `deploy/lib/`) handles all the complexity:

```typescript
// Example: What cargo pmcp deploy generates in CDK
// You don't write this - it's generated from deploy.toml

// User Pool with enterprise settings
const userPool = new cognito.UserPool(this, 'McpUserPool', {
  userPoolName: `${serverId}-users`,
  selfSignUpEnabled: false,  // Admin-only provisioning
  passwordPolicy: {
    minLength: 12,
    requireLowercase: true,
    requireUppercase: true,
    requireDigits: true,
    requireSymbols: true,
  },
  mfa: cognito.Mfa.OPTIONAL,
});

// Resource server with MCP scopes
const resourceServer = userPool.addResourceServer('McpApi', {
  identifier: 'mcp',
  scopes: [
    { scopeName: 'read', scopeDescription: 'Read MCP resources' },
    { scopeName: 'write', scopeDescription: 'Modify MCP resources' },
    { scopeName: 'admin', scopeDescription: 'Admin operations' },
  ],
});

// App client with OAuth configuration
const appClient = userPool.addClient('McpClient', {
  generateSecret: true,
  oAuth: {
    flows: { authorizationCodeGrant: true },
    scopes: [/* from deploy.toml */],
    callbackUrls: [/* from deploy.toml */],
  },
});
```

**The key insight:** You configure OAuth in `deploy.toml`, and the deployment tooling generates the correct CDK/CloudFormation. You don't need to understand Cognito's complex configuration options.

### Step 3: Your Server Code

Your Rust code just uses the OAuth middleware—it doesn't know or care that Cognito is the provider:

```rust
use pmcp::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // OAuth configuration is loaded from environment
    // (set by CDK stack outputs)
    let server = ServerBuilder::new("my-server", "1.0.0")
        .with_oauth_from_env()  // Reads COGNITO_* env vars
        .with_tool(MyTool)
        .build()?;

    server.serve().await
}
```

## Manual Setup (When You Need Control)

If you need more control, or your organization has specific Cognito requirements, you can configure Cognito manually. The rest of this chapter covers manual setup.

## Cognito Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cognito for MCP Servers                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐                                                │
│  │   User Pool     │  Authentication                                │
│  │  ─────────────  │  • User sign-up/sign-in                        │
│  │  • Users        │  • Password policies                           │
│  │  • Groups       │  • MFA                                         │
│  │  • App clients  │  • Custom attributes                           │
│  └────────┬────────┘  • Federation (SAML/OIDC)                      │
│           │                                                         │
│           │ Issues JWT                                              │
│           ▼                                                         │
│  ┌─────────────────┐                                                │
│  │   MCP Server    │  Validates JWT, extracts user info             │
│  └─────────────────┘                                                │
│                                                                     │
│  (Optional for AWS access)                                          │
│  ┌─────────────────┐                                                │
│  │  Identity Pool  │  AWS credentials for resources                 │
│  └─────────────────┘  • S3, DynamoDB, etc.                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Creating a User Pool

### AWS Console

1. Go to Cognito → User Pools → Create user pool
2. Configure sign-in:
   - Email as username (recommended)
   - Enable MFA (optional but recommended)
3. Configure sign-up:
   - Self-registration or admin-only
   - Required attributes (email)
4. Configure app client:
   - Create app client for your MCP server
   - Enable ALLOW_USER_SRP_AUTH
   - Generate client secret (for server-side apps)

### AWS CLI / CloudFormation

```yaml
# cloudformation/cognito.yaml
AWSTemplateFormatVersion: '2010-09-09'
Description: Cognito User Pool for MCP Server

Resources:
  UserPool:
    Type: AWS::Cognito::UserPool
    Properties:
      UserPoolName: mcp-server-users
      UsernameAttributes:
        - email
      AutoVerifiedAttributes:
        - email
      MfaConfiguration: OPTIONAL
      Policies:
        PasswordPolicy:
          MinimumLength: 12
          RequireLowercase: true
          RequireNumbers: true
          RequireSymbols: true
          RequireUppercase: true
      Schema:
        - Name: email
          Required: true
          Mutable: true
        - Name: department
          AttributeDataType: String
          Mutable: true

  UserPoolClient:
    Type: AWS::Cognito::UserPoolClient
    Properties:
      UserPoolId: !Ref UserPool
      ClientName: mcp-server-client
      GenerateSecret: true
      ExplicitAuthFlows:
        - ALLOW_USER_SRP_AUTH
        - ALLOW_REFRESH_TOKEN_AUTH
        - ALLOW_USER_PASSWORD_AUTH  # For testing only
      SupportedIdentityProviders:
        - COGNITO
      AllowedOAuthFlows:
        - code
      AllowedOAuthScopes:
        - openid
        - email
        - profile
        - mcp-server/read:tools
        - mcp-server/execute:tools
      AllowedOAuthFlowsUserPoolClient: true
      CallbackURLs:
        - https://your-app.com/callback
        - http://localhost:3000/callback

  ResourceServer:
    Type: AWS::Cognito::UserPoolResourceServer
    Properties:
      UserPoolId: !Ref UserPool
      Identifier: mcp-server
      Name: MCP Server API
      Scopes:
        - ScopeName: read:tools
          ScopeDescription: Read MCP tools
        - ScopeName: execute:tools
          ScopeDescription: Execute MCP tools
        - ScopeName: admin
          ScopeDescription: Admin operations

Outputs:
  UserPoolId:
    Value: !Ref UserPool
  UserPoolClientId:
    Value: !Ref UserPoolClient
  UserPoolDomain:
    Value: !Sub "https://cognito-idp.${AWS::Region}.amazonaws.com/${UserPool}"
```

## Rust Integration

### Configuration

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct CognitoConfig {
    pub region: String,
    pub user_pool_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl CognitoConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            region: env::var("AWS_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string()),
            user_pool_id: env::var("COGNITO_USER_POOL_ID")?,
            client_id: env::var("COGNITO_CLIENT_ID")?,
            client_secret: env::var("COGNITO_CLIENT_SECRET").ok(),
        })
    }

    pub fn issuer(&self) -> String {
        format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            self.region, self.user_pool_id
        )
    }

    pub fn jwks_uri(&self) -> String {
        format!("{}/.well-known/jwks.json", self.issuer())
    }
}
```

### Validator Setup

```rust
impl JwtValidatorConfig {
    pub fn from_cognito(config: &CognitoConfig) -> Self {
        Self {
            issuer: config.issuer(),
            audience: config.client_id.clone(),
            jwks_uri: config.jwks_uri(),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

// Main setup
pub async fn setup_cognito_auth() -> Result<JwtValidator> {
    let config = CognitoConfig::from_env()?;
    let validator_config = JwtValidatorConfig::from_cognito(&config);
    Ok(JwtValidator::new(validator_config))
}
```

### Cognito-Specific Claims

```rust
#[derive(Debug, Deserialize)]
pub struct CognitoClaims {
    // Standard claims
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,

    // Cognito-specific
    pub token_use: String,           // "access" or "id"
    pub auth_time: Option<u64>,
    pub client_id: Option<String>,

    // User attributes (from ID token)
    pub email: Option<String>,
    pub email_verified: Option<bool>,

    // Groups (custom claim)
    #[serde(rename = "cognito:groups")]
    pub groups: Option<Vec<String>>,

    #[serde(rename = "cognito:username")]
    pub username: Option<String>,

    // Custom attributes (prefixed with "custom:")
    #[serde(flatten)]
    pub custom_attributes: HashMap<String, serde_json::Value>,
}

impl CognitoClaims {
    pub fn get_custom(&self, name: &str) -> Option<&serde_json::Value> {
        self.custom_attributes.get(&format!("custom:{}", name))
    }

    pub fn is_access_token(&self) -> bool {
        self.token_use == "access"
    }

    pub fn is_id_token(&self) -> bool {
        self.token_use == "id"
    }
}
```

## Groups and Permissions

### Creating Groups

```bash
# Create groups in Cognito
aws cognito-idp create-group \
  --user-pool-id us-east-1_xxxx \
  --group-name Admins \
  --description "Administrator access"

aws cognito-idp create-group \
  --user-pool-id us-east-1_xxxx \
  --group-name Developers \
  --description "Developer access"

# Add user to group
aws cognito-idp admin-add-user-to-group \
  --user-pool-id us-east-1_xxxx \
  --username user@example.com \
  --group-name Developers
```

### Group-Based Authorization

```rust
impl AuthContext {
    pub fn from_cognito_claims(claims: &CognitoClaims) -> Self {
        let groups = claims.groups.clone().unwrap_or_default();

        // Map groups to scopes
        let mut scopes: HashSet<String> = HashSet::new();

        for group in &groups {
            match group.as_str() {
                "Admins" => {
                    scopes.insert("admin:*".into());
                    scopes.insert("execute:tools".into());
                    scopes.insert("read:tools".into());
                }
                "Developers" => {
                    scopes.insert("execute:tools".into());
                    scopes.insert("read:tools".into());
                }
                "ReadOnly" => {
                    scopes.insert("read:tools".into());
                }
                _ => {}
            }
        }

        Self {
            user_id: claims.sub.clone(),
            email: claims.email.clone(),
            name: claims.username.clone(),
            scopes,
        }
    }
}
```

## Federation with Corporate IdP

### SAML Federation

```yaml
# CloudFormation for SAML IdP
SAMLIdentityProvider:
  Type: AWS::Cognito::UserPoolIdentityProvider
  Properties:
    UserPoolId: !Ref UserPool
    ProviderName: CorporateSSO
    ProviderType: SAML
    ProviderDetails:
      MetadataURL: https://idp.company.com/metadata.xml
    AttributeMapping:
      email: http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress
      given_name: http://schemas.xmlsoap.org/ws/2005/05/identity/claims/givenname
      family_name: http://schemas.xmlsoap.org/ws/2005/05/identity/claims/surname
      custom:department: Department
```

### OIDC Federation

```yaml
OIDCIdentityProvider:
  Type: AWS::Cognito::UserPoolIdentityProvider
  Properties:
    UserPoolId: !Ref UserPool
    ProviderName: Okta
    ProviderType: OIDC
    ProviderDetails:
      client_id: okta-client-id
      client_secret: okta-client-secret
      authorize_scopes: openid email profile
      oidc_issuer: https://company.okta.com
    AttributeMapping:
      email: email
      given_name: given_name
      family_name: family_name
```

## Lambda Triggers

Customize authentication with Lambda triggers:

```rust
// Pre-token generation trigger - customize JWT claims
use aws_lambda_events::event::cognito::CognitoEventUserPoolsPreTokenGen;
use lambda_runtime::{service_fn, Error, LambdaEvent};

async fn pre_token_gen(
    event: LambdaEvent<CognitoEventUserPoolsPreTokenGen>,
) -> Result<CognitoEventUserPoolsPreTokenGen, Error> {
    let mut response = event.payload;

    // Add custom claims based on user attributes
    let user_attributes = &response.request.user_attributes;

    if let Some(department) = user_attributes.get("custom:department") {
        // Add department to claims
        response.response.claims_override_details
            .get_or_insert_default()
            .claims_to_add_or_override
            .get_or_insert_default()
            .insert("department".into(), department.clone());
    }

    // Add permissions based on groups
    if let Some(groups) = user_attributes.get("cognito:groups") {
        let permissions = groups_to_permissions(groups);
        response.response.claims_override_details
            .get_or_insert_default()
            .claims_to_add_or_override
            .get_or_insert_default()
            .insert("permissions".into(), permissions.join(" "));
    }

    Ok(response)
}
```

## Testing with Cognito

### Get Test Token

```bash
# Create test user
aws cognito-idp admin-create-user \
  --user-pool-id us-east-1_xxxx \
  --username testuser@example.com \
  --user-attributes Name=email,Value=testuser@example.com \
  --temporary-password TempPass123!

# Set permanent password
aws cognito-idp admin-set-user-password \
  --user-pool-id us-east-1_xxxx \
  --username testuser@example.com \
  --password SecurePass123! \
  --permanent

# Get tokens
aws cognito-idp admin-initiate-auth \
  --user-pool-id us-east-1_xxxx \
  --client-id your-client-id \
  --auth-flow ADMIN_USER_PASSWORD_AUTH \
  --auth-parameters USERNAME=testuser@example.com,PASSWORD=SecurePass123!
```

### Integration Test

```rust
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_cognito_auth() {
    let config = CognitoConfig::from_env().unwrap();
    let validator = JwtValidator::new(JwtValidatorConfig::from_cognito(&config));

    // Get token via AWS SDK
    let token = get_cognito_token(&config).await.unwrap();

    let claims = validator.validate(&token).await.unwrap();

    assert!(!claims.sub.is_empty());
    println!("User ID: {}", claims.sub);
    println!("Email: {:?}", claims.email);
}

async fn get_cognito_token(config: &CognitoConfig) -> Result<String> {
    let client = aws_sdk_cognitoidentityprovider::Client::new(&aws_config::load_from_env().await);

    let response = client
        .admin_initiate_auth()
        .user_pool_id(&config.user_pool_id)
        .client_id(&config.client_id)
        .auth_flow(AuthFlowType::AdminUserPasswordAuth)
        .auth_parameters("USERNAME", "testuser@example.com")
        .auth_parameters("PASSWORD", "SecurePass123!")
        .send()
        .await?;

    Ok(response.authentication_result()
        .unwrap()
        .access_token()
        .unwrap()
        .to_string())
}
```

## Summary

**Recommended approach:** Use `cargo pmcp deploy init --oauth cognito` to generate the CDK stack that handles all Cognito complexity. You configure scopes in `deploy.toml`, and the deployment creates the user pool, app client, and resource server automatically.

**If you need manual setup**, AWS Cognito integration requires:

1. **User Pool** - Authentication and user management
2. **App Client** - OAuth configuration
3. **Resource Server** - Custom scopes
4. **Groups** - Permission management
5. **Federation** - Corporate IdP integration (optional)

Key Cognito-specific considerations:
- Token types: Access vs ID tokens
- Groups appear in `cognito:groups` claim
- Custom attributes prefixed with `custom:`
- Lambda triggers for claim customization

**Remember:** Cognito is just one option. If your organization uses Okta, Auth0, Entra ID, or another provider, use that instead—the patterns are the same.

---

*Continue to [Auth0](./ch14-02-auth0.md) →*
