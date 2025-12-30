# Microsoft Entra ID

Microsoft Entra ID (formerly Azure Active Directory) is the identity platform for Microsoft 365 enterprises. This chapter covers Entra ID integration for MCP servers.

## Entra ID Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Entra ID for MCP Servers                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Microsoft 365 Tenant                                               │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                                                              │   │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │   │
│  │  │    Users      │  │    Groups     │  │  App Roles    │   │   │
│  │  │  (Employees)  │  │ (AD Groups)   │  │  (Defined in  │   │   │
│  │  │              │  │               │  │   App Reg)    │   │   │
│  │  └───────────────┘  └───────────────┘  └───────────────┘   │   │
│  │                                                              │   │
│  │  App Registration (MCP Server)                               │   │
│  │  ├─ Client ID                                               │   │
│  │  ├─ API permissions                                         │   │
│  │  └─ App roles                                               │   │
│  │                                                              │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Token Flow:                                                        │
│  User → Entra ID → JWT with oid, groups, roles → MCP Server        │
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

Microsoft Entra ID integration requires:

1. **App Registration** - Client ID, tenant ID, app roles
2. **Token Configuration** - Optional claims for user info
3. **App Roles** - Permission model for your application
4. **Group Claims** - Map AD groups to permissions

Key Entra-specific considerations:
- Use `oid` (Object ID) as the stable user identifier
- Roles appear in `roles` array (from app roles)
- Groups are GUIDs, need mapping to permissions
- Multi-tenant requires special issuer validation
- 5-minute clock skew recommended

---

*Continue to [Multi-Tenant Considerations](./ch14-04-multitenant.md) →*
