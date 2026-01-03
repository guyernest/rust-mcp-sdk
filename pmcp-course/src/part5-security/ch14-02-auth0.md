# Auth0

Auth0 is a flexible identity platform known for developer-friendly APIs and extensive customization. This chapter covers Auth0 integration for MCP servers.

> **Note:** Auth0 is shown here as an example. If your organization already uses a different identity provider (Okta, Cognito, Entra ID, etc.), use that instead. The patterns in this chapter apply to any OIDC-compliant provider.

## The Easy Way: `cargo pmcp` + CDK

**The fastest path to production:** Use `cargo pmcp` to configure OAuth with Auth0. The deployment handles API and application setup.

### Step 1: Initialize OAuth Configuration

```bash
# Initialize deployment with Auth0 OAuth
cargo pmcp deploy init --target pmcp-run --oauth auth0

# This creates/updates .pmcp/deploy.toml with:
```

```toml
# .pmcp/deploy.toml
[auth]
enabled = true
provider = "auth0"
domain = "your-tenant.auth0.com"  # Set this to your Auth0 domain
audience = "https://mcp.example.com"  # Your API identifier

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
    "read:tools",
]
allowed_scopes = [
    "openid",
    "email",
    "profile",
    "read:tools",
    "execute:tools",
    "read:resources",
    "write:resources",
    "admin",
]
```

### Step 2: Configure Auth0 (One-Time Setup)

Unlike Cognito, Auth0 isn't created by CDK—you configure it in Auth0 Dashboard. But `cargo pmcp` generates the correct values:

```bash
# After running deploy init, it outputs:
#
# Auth0 Setup Required:
# 1. Create API in Auth0 Dashboard
#    - Identifier: https://mcp.example.com
#    - Permissions: read:tools, execute:tools, read:resources, write:resources, admin
#
# 2. Create Application (Regular Web Application)
#    - Callback URLs: https://your-deployment.pmcp.run/callback
#
# 3. Set environment variables or update deploy.toml:
#    AUTH0_DOMAIN=your-tenant.auth0.com
#    AUTH0_AUDIENCE=https://mcp.example.com
```

### Step 3: Deploy

```bash
# Build and deploy
cargo pmcp deploy

# The deployment:
# - Configures Lambda with Auth0 environment variables
# - Sets up JWT validation middleware
# - Your server validates Auth0 tokens automatically
```

### Step 4: Your Server Code

Your Rust code is provider-agnostic—it just uses OAuth middleware:

```rust
use pmcp::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // OAuth configuration loaded from environment
    // (AUTH0_DOMAIN, AUTH0_AUDIENCE set by deployment)
    let server = ServerBuilder::new("my-server", "1.0.0")
        .with_oauth_from_env()  // Works with any provider
        .with_tool(MyTool)
        .build()?;

    server.serve().await
}
```

## Manual Setup (When You Need Control)

If you need more control over Auth0 configuration, or your organization has specific Auth0 requirements, you can configure it manually. The rest of this chapter covers manual setup.

## Auth0 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Auth0 for MCP Servers                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐    ┌─────────────────┐                         │
│  │   Application   │    │      API        │                         │
│  │  (MCP Client)   │    │  (MCP Server)   │                         │
│  └────────┬────────┘    └────────┬────────┘                         │
│           │                      │                                  │
│           │ Auth Code Flow       │ Validates JWT                    │
│           ▼                      │                                  │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                         Auth0 Tenant                        │    │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │    │
│  │  │  Connections  │  │    Rules/     │  │     RBAC      │    │    │
│  │  │  (Database,   │  │   Actions     │  │ (Roles &      │    │    │
│  │  │  Social,      │  │  (Customize   │  │  Permissions) │    │    │
│  │  │  Enterprise)  │  │   tokens)     │  │               │    │    │
│  │  └───────────────┘  └───────────────┘  └───────────────┘    │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Setting Up Auth0

### Create Tenant and API

1. **Create Auth0 account** at auth0.com
2. **Create API** (represents your MCP server):
   - Go to Applications → APIs → Create API
   - Name: "MCP Server"
   - Identifier: `https://mcp.example.com` (your audience)
   - Signing Algorithm: RS256

3. **Create Application** (represents MCP clients):
   - Go to Applications → Create Application
   - Type: Regular Web Application (for server-side)
   - Configure callback URLs

### Define Permissions

```json
// API Settings → Permissions
{
  "permissions": [
    { "value": "read:tools", "description": "List and describe tools" },
    { "value": "execute:tools", "description": "Execute MCP tools" },
    { "value": "read:resources", "description": "Read resources" },
    { "value": "write:resources", "description": "Modify resources" },
    { "value": "admin", "description": "Administrative access" }
  ]
}
```

### Create Roles

```json
// User Management → Roles
{
  "roles": [
    {
      "name": "MCP User",
      "permissions": ["read:tools", "execute:tools", "read:resources"]
    },
    {
      "name": "MCP Admin",
      "permissions": ["read:tools", "execute:tools", "read:resources", "write:resources", "admin"]
    }
  ]
}
```

## Rust Integration

### Configuration

```rust
#[derive(Debug, Clone)]
pub struct Auth0Config {
    pub domain: String,
    pub audience: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl Auth0Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            domain: env::var("AUTH0_DOMAIN")?,
            audience: env::var("AUTH0_AUDIENCE")?,
            client_id: env::var("AUTH0_CLIENT_ID")?,
            client_secret: env::var("AUTH0_CLIENT_SECRET").ok(),
        })
    }

    pub fn issuer(&self) -> String {
        format!("https://{}/", self.domain)
    }

    pub fn jwks_uri(&self) -> String {
        format!("https://{}/.well-known/jwks.json", self.domain)
    }

    pub fn token_endpoint(&self) -> String {
        format!("https://{}/oauth/token", self.domain)
    }
}
```

### Validator Setup

```rust
impl JwtValidatorConfig {
    pub fn from_auth0(config: &Auth0Config) -> Self {
        Self {
            issuer: config.issuer(),
            audience: config.audience.clone(),
            jwks_uri: config.jwks_uri(),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}
```

### Auth0 Claims

```rust
#[derive(Debug, Deserialize)]
pub struct Auth0Claims {
    // Standard OIDC claims
    pub sub: String,              // "auth0|123" or "google-oauth2|456"
    pub iss: String,
    pub aud: ClaimAudience,
    pub exp: u64,
    pub iat: u64,
    pub azp: Option<String>,      // Authorized party (client_id)

    // User info
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub nickname: Option<String>,
    pub picture: Option<String>,

    // RBAC permissions (requires API setting)
    pub permissions: Option<Vec<String>>,

    // Scope string
    pub scope: Option<String>,

    // Custom claims (namespaced)
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

impl Auth0Claims {
    /// Get namespaced custom claim
    pub fn get_custom(&self, namespace: &str, key: &str) -> Option<&serde_json::Value> {
        self.custom.get(&format!("{}/{}", namespace, key))
    }

    /// Get permissions (from RBAC or scope)
    pub fn permissions_list(&self) -> Vec<String> {
        self.permissions.clone().unwrap_or_else(|| {
            self.scope
                .as_ref()
                .map(|s| s.split_whitespace().map(String::from).collect())
                .unwrap_or_default()
        })
    }

    /// Parse identity provider from sub claim
    pub fn identity_provider(&self) -> &str {
        self.sub.split('|').next().unwrap_or("unknown")
    }
}
```

## Role-Based Access Control (RBAC)

### Enable RBAC

In Auth0 Dashboard → APIs → Your API → Settings:
- Enable RBAC: ON
- Add Permissions in the Access Token: ON

### Permissions in Token

With RBAC enabled, permissions appear in the access token:

```json
{
  "iss": "https://your-tenant.auth0.com/",
  "sub": "auth0|123456",
  "aud": "https://mcp.example.com",
  "permissions": [
    "read:tools",
    "execute:tools",
    "read:resources"
  ]
}
```

### Authorization in Rust

```rust
impl AuthContext {
    pub fn from_auth0_claims(claims: &Auth0Claims) -> Self {
        Self {
            user_id: claims.sub.clone(),
            email: claims.email.clone(),
            name: claims.name.clone(),
            scopes: claims.permissions_list().into_iter().collect(),
        }
    }
}

// Use in tools
pub async fn run(&self, input: Input, context: &ToolContext) -> Result<Output> {
    let auth = context.auth()?;

    // Check for specific permission
    auth.require_scope("execute:tools")?;

    // Or check any of multiple permissions
    auth.require_any_scope(&["admin", "write:resources"])?;

    // Proceed with operation
}
```

## Auth0 Actions

### Customize Tokens with Actions

```javascript
// Actions → Flows → Login → Add Action

exports.onExecutePostLogin = async (event, api) => {
  // Add custom claims (must be namespaced)
  const namespace = 'https://mcp.example.com';

  // Add user metadata
  if (event.user.app_metadata.department) {
    api.accessToken.setCustomClaim(
      `${namespace}/department`,
      event.user.app_metadata.department
    );
  }

  // Add organization info
  if (event.organization) {
    api.accessToken.setCustomClaim(
      `${namespace}/org_id`,
      event.organization.id
    );
    api.accessToken.setCustomClaim(
      `${namespace}/org_name`,
      event.organization.name
    );
  }

  // Add custom permissions based on conditions
  if (event.user.email.endsWith('@admin.example.com')) {
    // Get existing permissions
    const permissions = event.authorization?.permissions || [];
    permissions.push('admin:*');
    api.accessToken.setCustomClaim('permissions', permissions);
  }
};
```

### Handle Custom Claims in Rust

```rust
impl Auth0Claims {
    pub fn department(&self) -> Option<String> {
        self.get_custom("https://mcp.example.com", "department")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    pub fn org_id(&self) -> Option<String> {
        self.get_custom("https://mcp.example.com", "org_id")
            .and_then(|v| v.as_str())
            .map(String::from)
    }
}
```

## Enterprise Connections

### SAML Connection

1. Go to Authentication → Enterprise → SAML
2. Create connection with IdP metadata
3. Map attributes:

```json
{
  "mappings": {
    "email": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
    "given_name": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/givenname",
    "family_name": "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/surname",
    "department": "Department",
    "groups": "Groups"
  }
}
```

### Azure AD Connection

For Microsoft enterprise users:

1. Authentication → Enterprise → Microsoft Azure AD
2. Configure with Azure tenant ID and client credentials
3. Enable in your application

## Organizations (Multi-Tenant)

Auth0 Organizations support B2B multi-tenancy:

```javascript
// Enable organizations in Auth0 Dashboard
// Applications → Your App → Organizations → Enable

// Token will include organization claim
{
  "org_id": "org_abc123",
  "org_name": "Acme Corp"
}
```

```rust
impl Auth0Claims {
    pub fn organization(&self) -> Option<(String, Option<String>)> {
        let org_id = self.custom.get("org_id")
            .and_then(|v| v.as_str())
            .map(String::from)?;

        let org_name = self.custom.get("org_name")
            .and_then(|v| v.as_str())
            .map(String::from);

        Some((org_id, org_name))
    }
}
```

## Testing with Auth0

### Get Test Token (Password Grant)

```bash
# Enable Password grant in Application settings first
curl --request POST \
  --url 'https://your-tenant.auth0.com/oauth/token' \
  --header 'content-type: application/x-www-form-urlencoded' \
  --data grant_type=password \
  --data 'username=test@example.com' \
  --data 'password=TestPass123!' \
  --data 'client_id=YOUR_CLIENT_ID' \
  --data 'client_secret=YOUR_CLIENT_SECRET' \
  --data 'audience=https://mcp.example.com' \
  --data 'scope=openid email profile'
```

### Get Test Token (Client Credentials)

```bash
# For machine-to-machine testing
curl --request POST \
  --url 'https://your-tenant.auth0.com/oauth/token' \
  --header 'content-type: application/x-www-form-urlencoded' \
  --data grant_type=client_credentials \
  --data 'client_id=YOUR_CLIENT_ID' \
  --data 'client_secret=YOUR_CLIENT_SECRET' \
  --data 'audience=https://mcp.example.com'
```

### Integration Test

```rust
#[tokio::test]
#[ignore]
async fn test_auth0_validation() {
    let config = Auth0Config::from_env().unwrap();
    let validator = JwtValidator::new(JwtValidatorConfig::from_auth0(&config));

    // Get token
    let token = get_auth0_token(&config).await.unwrap();

    // Validate
    let claims = validator.validate(&token).await.unwrap();

    assert!(!claims.sub.is_empty());
    println!("User: {}", claims.sub);
    println!("Permissions: {:?}", claims.permissions);
}

async fn get_auth0_token(config: &Auth0Config) -> Result<String> {
    let client = reqwest::Client::new();

    let response: serde_json::Value = client
        .post(&config.token_endpoint())
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", &config.client_id),
            ("client_secret", config.client_secret.as_ref().unwrap()),
            ("audience", &config.audience),
        ])
        .send()
        .await?
        .json()
        .await?;

    Ok(response["access_token"].as_str().unwrap().to_string())
}
```

## Summary

**Recommended approach:** Use `cargo pmcp deploy init --oauth auth0` to generate deployment configuration. You'll need to create the API and Application in Auth0 Dashboard (one-time setup), then `cargo pmcp deploy` handles the rest.

**If you need manual setup**, Auth0 integration provides:

1. **Applications** - OAuth clients for your MCP consumers
2. **APIs** - Define audience and permissions
3. **RBAC** - Role-based permission management
4. **Actions** - Customize tokens with business logic
5. **Organizations** - Multi-tenant support
6. **Connections** - Enterprise IdP federation

Key Auth0-specific considerations:
- Permissions via RBAC appear in `permissions` array
- Custom claims require namespacing (e.g., `https://mcp.example.com/claim`)
- `sub` format: `provider|id` (e.g., `auth0|123`, `google-oauth2|456`)
- Actions for advanced token customization

**Remember:** Auth0 is just one option. If your organization uses Okta, Cognito, Entra ID, or another provider, use that instead—the patterns are the same.

---

*Continue to [Microsoft Entra ID](./ch14-03-entra.md) →*
