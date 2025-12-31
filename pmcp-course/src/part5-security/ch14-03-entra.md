# Microsoft Entra ID

Microsoft Entra ID (formerly Azure Active Directory) is the identity platform for Microsoft 365 enterprises. This chapter covers Entra ID integration for MCP servers.

> **Note:** Entra ID is shown here as an example. If your organization already uses a different identity provider (Okta, Auth0, Cognito, etc.), use that instead. The patterns in this chapter apply to any OIDC-compliant provider. However, if your organization is a Microsoft 365 shop, Entra ID is likely your best choice—it's what your employees already use.

## The Easy Way: `cargo pmcp` + CDK

**The fastest path to production:** Use `cargo pmcp` to configure OAuth with Entra ID. Your server validates tokens automatically.

### Step 1: Initialize OAuth Configuration

```bash
# Initialize deployment with Entra ID OAuth
cargo pmcp deploy init --target pmcp-run --oauth entra

# This creates/updates .pmcp/deploy.toml with:
```

```toml
# .pmcp/deploy.toml
[auth]
enabled = true
provider = "entra"
tenant_id = "your-tenant-id"  # From Azure Portal
client_id = "your-client-id"  # From App Registration

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
    "profile",
]
```

### Step 2: Configure Entra ID (One-Time Setup)

Entra ID resources are managed in Azure Portal. `cargo pmcp` tells you what to create:

```bash
# After running deploy init, it outputs:
#
# Entra ID Setup Required:
# 1. Create App Registration in Azure Portal
#    - Go to: Entra ID → App registrations → New registration
#    - Name: "MCP Server - Production"
#    - Redirect URI: https://your-deployment.pmcp.run/callback
#
# 2. Configure App Roles (App registration → App roles):
#    - MCP.User: Can read and execute tools
#    - MCP.Admin: Full administrative access
#
# 3. Configure Token (App registration → Token configuration):
#    - Add optional claims: email, groups
#
# 4. Record these values for deploy.toml:
#    - Application (client) ID
#    - Directory (tenant) ID
#
# 5. Set environment variables or update deploy.toml:
#    ENTRA_TENANT_ID=your-tenant-id
#    ENTRA_CLIENT_ID=your-client-id
```

### Step 3: Deploy

```bash
# Build and deploy
cargo pmcp deploy

# The deployment:
# - Configures Lambda with Entra ID environment variables
# - Sets up JWT validation middleware with correct issuer/JWKS
# - Your server validates Entra ID tokens automatically
```

### Step 4: Your Server Code

Your Rust code is provider-agnostic:

```rust
use pmcp::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // OAuth configuration loaded from environment
    // (ENTRA_TENANT_ID, ENTRA_CLIENT_ID set by deployment)
    let server = ServerBuilder::new("my-server", "1.0.0")
        .with_oauth_from_env()  // Works with any provider
        .with_tool(MyTool)
        .build()?;

    server.serve().await
}
```

### Why Entra ID for Microsoft Shops

If your organization uses Microsoft 365, Entra ID is the natural choice:

| Benefit | Description |
|---------|-------------|
| **Same login** | Employees use their Microsoft 365 credentials |
| **AD groups** | Existing Active Directory groups work for MCP permissions |
| **SSO everywhere** | MCP access works like Teams, Outlook, SharePoint |
| **IT familiarity** | Your IT team already knows Entra ID |
| **Conditional Access** | Apply existing security policies to MCP |

## Manual Setup (When You Need Control)

If you need more control over Entra ID configuration, or your organization has specific requirements (custom claims, complex group mappings, on-behalf-of flows), you can configure it manually. The rest of this chapter covers manual setup.

## Entra ID Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Entra ID for MCP Servers                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Microsoft 365 Tenant                                               │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                                                             │    │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │    │
│  │  │    Users      │  │    Groups     │  │  App Roles    │    │    │
│  │  │  (Employees)  │  │ (AD Groups)   │  │  (Defined in  │    │    │
│  │  │               │  │               │  │   App Reg)    │    │    │
│  │  └───────────────┘  └───────────────┘  └───────────────┘    │    │
│  │                                                             │    │
│  │  App Registration (MCP Server)                              │    │
│  │  ├─ Client ID                                               │    │
│  │  ├─ API permissions                                         │    │
│  │  └─ App roles                                               │    │
│  │                                                             │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  Token Flow:                                                        │
│  User → Entra ID → JWT with oid, groups, roles → MCP Server         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Setting Up App Registration

### Azure Portal

1. **Go to** Entra ID → App registrations → New registration
2. **Configure**:
   - Name: "MCP Server"
   - Supported account types: Single tenant or multi-tenant
   - Redirect URI: Web, `https://your-app/callback`

3. **Record**:
   - Application (client) ID
   - Directory (tenant) ID

### Define App Roles

In App registration → App roles → Create app role:

```json
{
  "appRoles": [
    {
      "displayName": "MCP User",
      "value": "MCP.User",
      "description": "Can read and execute MCP tools",
      "allowedMemberTypes": ["User"]
    },
    {
      "displayName": "MCP Admin",
      "value": "MCP.Admin",
      "description": "Full administrative access",
      "allowedMemberTypes": ["User"]
    },
    {
      "displayName": "MCP Service",
      "value": "MCP.Service",
      "description": "Machine-to-machine access",
      "allowedMemberTypes": ["Application"]
    }
  ]
}
```

### Configure Token

In App registration → Token configuration:

1. Add optional claims:
   - `email`
   - `given_name`
   - `family_name`
   - `groups` (group membership)

2. For groups claim, configure:
   - Emit groups as role claims: Security groups

## Rust Integration

### Configuration

```rust
#[derive(Debug, Clone)]
pub struct EntraConfig {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl EntraConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            tenant_id: env::var("ENTRA_TENANT_ID")
                .or_else(|_| env::var("AZURE_TENANT_ID"))?,
            client_id: env::var("ENTRA_CLIENT_ID")
                .or_else(|_| env::var("AZURE_CLIENT_ID"))?,
            client_secret: env::var("ENTRA_CLIENT_SECRET")
                .or_else(|_| env::var("AZURE_CLIENT_SECRET"))
                .ok(),
        })
    }

    pub fn issuer(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/v2.0",
            self.tenant_id
        )
    }

    pub fn jwks_uri(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/discovery/v2.0/keys",
            self.tenant_id
        )
    }

    pub fn token_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        )
    }
}
```

### Validator Setup

```rust
impl JwtValidatorConfig {
    pub fn from_entra(config: &EntraConfig) -> Self {
        Self {
            issuer: config.issuer(),
            audience: config.client_id.clone(),
            jwks_uri: config.jwks_uri(),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 300, // Entra recommends 5 minutes
        }
    }
}
```

### Entra ID Claims

```rust
#[derive(Debug, Deserialize)]
pub struct EntraClaims {
    // Standard claims
    pub sub: String,
    pub iss: String,
    pub aud: ClaimAudience,
    pub exp: u64,
    pub iat: u64,
    pub nbf: u64,

    // Entra-specific identifiers
    pub oid: String,                      // Object ID (user GUID)
    pub tid: String,                      // Tenant ID
    pub azp: Option<String>,              // Authorized party (client_id)

    // User info
    pub preferred_username: Option<String>, // UPN (user@domain.com)
    pub email: Option<String>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,

    // Groups (GUIDs)
    pub groups: Option<Vec<String>>,

    // App roles (from app registration)
    pub roles: Option<Vec<String>>,

    // For multi-tenant apps
    pub idp: Option<String>,              // Identity provider
}

impl EntraClaims {
    /// User's UPN (email-like identifier)
    pub fn upn(&self) -> Option<&str> {
        self.preferred_username.as_deref()
            .or(self.email.as_deref())
    }

    /// Primary identifier - use oid for consistency
    pub fn user_id(&self) -> &str {
        &self.oid
    }

    /// Check if user has specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.as_ref()
            .map(|r| r.iter().any(|r| r == role))
            .unwrap_or(false)
    }

    /// Check if user is in group (by GUID)
    pub fn in_group(&self, group_id: &str) -> bool {
        self.groups.as_ref()
            .map(|g| g.iter().any(|g| g == group_id))
            .unwrap_or(false)
    }
}
```

## Group-Based Authorization

### Map Groups to Permissions

```rust
pub struct GroupPermissionMapper {
    // Map group GUIDs to permission sets
    group_permissions: HashMap<String, Vec<String>>,
}

impl GroupPermissionMapper {
    pub fn new() -> Self {
        let mut map = HashMap::new();

        // Configure your group mappings
        map.insert(
            "12345678-1234-1234-1234-123456789abc".into(), // Admins group GUID
            vec!["admin:*".into(), "execute:tools".into(), "read:tools".into()],
        );
        map.insert(
            "87654321-4321-4321-4321-cba987654321".into(), // Developers group
            vec!["execute:tools".into(), "read:tools".into()],
        );

        Self { group_permissions: map }
    }

    pub fn permissions_for_groups(&self, groups: &[String]) -> HashSet<String> {
        groups.iter()
            .filter_map(|g| self.group_permissions.get(g))
            .flatten()
            .cloned()
            .collect()
    }
}

impl AuthContext {
    pub fn from_entra_claims(claims: &EntraClaims, mapper: &GroupPermissionMapper) -> Self {
        let mut scopes: HashSet<String> = HashSet::new();

        // Add role-based permissions
        if let Some(roles) = &claims.roles {
            for role in roles {
                match role.as_str() {
                    "MCP.Admin" => {
                        scopes.insert("admin:*".into());
                    }
                    "MCP.User" => {
                        scopes.insert("execute:tools".into());
                        scopes.insert("read:tools".into());
                    }
                    _ => {}
                }
            }
        }

        // Add group-based permissions
        if let Some(groups) = &claims.groups {
            scopes.extend(mapper.permissions_for_groups(groups));
        }

        Self {
            user_id: claims.oid.clone(),
            email: claims.upn().map(String::from),
            name: claims.name.clone(),
            scopes,
        }
    }
}
```

## On-Behalf-Of Flow

For services that need to call other APIs on behalf of the user:

```rust
pub async fn get_obo_token(
    config: &EntraConfig,
    user_token: &str,
    target_scope: &str,
) -> Result<String> {
    let client = reqwest::Client::new();

    let response: serde_json::Value = client
        .post(&config.token_endpoint())
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("client_id", &config.client_id),
            ("client_secret", config.client_secret.as_ref().unwrap()),
            ("assertion", user_token),
            ("scope", target_scope),
            ("requested_token_use", "on_behalf_of"),
        ])
        .send()
        .await?
        .json()
        .await?;

    Ok(response["access_token"].as_str().unwrap().to_string())
}
```

## Multi-Tenant Applications

For apps serving multiple organizations:

### Configure Multi-Tenant

In App registration:
- Supported account types: "Accounts in any organizational directory"

### Validate Any Tenant

```rust
impl JwtValidatorConfig {
    pub fn from_entra_multitenant(client_id: &str) -> Self {
        Self {
            // Use 'common' endpoint for multi-tenant
            issuer: "https://login.microsoftonline.com/{tenantid}/v2.0".into(),
            audience: client_id.to_string(),
            jwks_uri: "https://login.microsoftonline.com/common/discovery/v2.0/keys".into(),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 300,
        }
    }
}

// Custom validation for multi-tenant
impl JwtValidator {
    pub async fn validate_multitenant(&self, token: &str) -> Result<EntraClaims> {
        let claims: EntraClaims = self.decode_without_validation(token)?;

        // Verify issuer matches tenant in token
        let expected_issuer = format!(
            "https://login.microsoftonline.com/{}/v2.0",
            claims.tid
        );
        if claims.iss != expected_issuer {
            return Err(AuthError::ValidationFailed("Invalid issuer".into()));
        }

        // Continue with normal validation
        self.validate(token).await
    }
}
```

## Testing with Entra ID

### Azure CLI

```bash
# Login
az login

# Get token for your app
az account get-access-token \
  --resource api://your-client-id \
  --query accessToken -o tsv
```

### Client Credentials (Service Principal)

```bash
curl -X POST \
  "https://login.microsoftonline.com/${TENANT_ID}/oauth2/v2.0/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "client_id=${CLIENT_ID}" \
  -d "client_secret=${CLIENT_SECRET}" \
  -d "scope=api://${CLIENT_ID}/.default" \
  -d "grant_type=client_credentials"
```

### Integration Test

```rust
#[tokio::test]
#[ignore]
async fn test_entra_validation() {
    let config = EntraConfig::from_env().unwrap();
    let validator = JwtValidator::new(JwtValidatorConfig::from_entra(&config));

    // Get token via Azure SDK or CLI
    let token = get_entra_token(&config).await.unwrap();

    let claims = validator.validate(&token).await.unwrap();

    assert!(!claims.sub.is_empty());
    println!("User OID: {}", claims.oid);
    println!("UPN: {:?}", claims.preferred_username);
    println!("Roles: {:?}", claims.roles);
}
```

## Summary

**Recommended approach:** Use `cargo pmcp deploy init --oauth entra` to generate deployment configuration. Create the App Registration in Azure Portal (one-time setup), then `cargo pmcp deploy` handles the rest.

**If your organization uses Microsoft 365:** Entra ID is your best choice. Employees use their existing credentials, IT uses familiar tools, and existing AD groups translate to MCP permissions.

**If you need manual setup**, Microsoft Entra ID integration requires:

1. **App Registration** - Client ID, tenant ID, app roles
2. **Token Configuration** - Optional claims for user info
3. **App Roles** - Permission model for your application
4. **Group Claims** - Map AD groups to permissions

Key Entra-specific considerations:
- Use `oid` (Object ID) as the stable user identifier, not `sub`
- Roles appear in `roles` array (from app roles you define)
- Groups are GUIDs—you need to map them to human-readable permissions
- Multi-tenant apps require special issuer validation (tenant ID varies)
- 5-minute clock skew recommended (Entra's guidance)

**Remember:** Entra ID is just one option. If your organization uses Okta, Auth0, Cognito, or another provider, use that instead—the patterns are the same.

---

*Continue to [Multi-Tenant Considerations](./ch14-04-multitenant.md) →*
