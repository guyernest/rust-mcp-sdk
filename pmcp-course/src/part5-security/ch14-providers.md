# Identity Provider Integration

This chapter covers integrating MCP servers with enterprise identity providers. We focus on the three most common enterprise IdPs: AWS Cognito, Auth0, and Microsoft Entra ID.

## The Most Important Advice: Use What You Already Have

**The providers in this course are examples, not recommendations.** The best identity provider for your MCP server is the one your organization already uses.

```
┌─────────────────────────────────────────────────────────────────────┐
│                 Provider Selection Decision Tree                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Does your organization already have an identity provider?          │
│                                                                     │
│  YES ──────────────────────────────────────────────────────────────▶│
│  │                                                                  │
│  │   USE THAT PROVIDER.                                             │
│  │                                                                  │
│  │   • Users already know how to log in                             │
│  │   • IT already knows how to manage it                            │
│  │   • Security policies already exist                              │
│  │   • Compliance is already handled                                │
│  │   • No new vendor relationships needed                           │
│  │                                                                  │
│  └──────────────────────────────────────────────────────────────────│
│                                                                     │
│  NO (starting fresh) ──────────────────────────────────────────────▶│
│  │                                                                  │
│  │   Consider your existing infrastructure:                         │
│  │   • Heavy AWS user? → Cognito                                    │
│  │   • Microsoft 365? → Entra ID                                    │
│  │   • Need flexibility? → Auth0 or Okta                            │
│  │   • Self-hosted? → Keycloak                                      │
│  │                                                                  │
│  └──────────────────────────────────────────────────────────────────│
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Why not switch providers for MCP?**

| Reason | Impact |
|--------|--------|
| Users have to learn new login | Friction, support tickets |
| IT has to manage two systems | Operational burden |
| Permissions need duplication | Security gaps |
| Compliance scope expands | More audit work |
| More vendors to manage | Procurement complexity |

**The code in this chapter works with any OAuth 2.0 / OIDC provider.** We use Cognito, Auth0, and Entra as examples because they're common, but the patterns apply to Okta, Keycloak, Google Identity, PingIdentity, or any other OIDC-compliant provider.

## Understanding Provider Examples

The providers covered in this chapter:

| Provider | Why We Cover It | Your Situation |
|----------|-----------------|----------------|
| **AWS Cognito** | Common in AWS shops | Use if you're already AWS-native |
| **Auth0** | Developer-friendly, good docs | Use if you need rapid prototyping |
| **Microsoft Entra** | Enterprise Microsoft environments | Use if you have Microsoft 365 |
| **Okta** | Enterprise workforce identity | Use if already deployed |
| **Keycloak** | Self-hosted, open source | Use if you need on-premises |

**If your organization uses a provider not listed here:** The patterns are the same. You need:
1. JWKS URI (for public keys)
2. Issuer URL (for token validation)
3. Audience value (your app identifier)
4. Understanding of how claims are structured

## Provider Feature Comparison

```
┌─────────────────────────────────────────────────────────────────────┐
│                  Identity Provider Comparison                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  AWS Cognito                                                        │
│  ├─ Best for: AWS-native applications                               │
│  ├─ Pros: Deep AWS integration, pay-per-use pricing                 │
│  ├─ Cons: Limited customization, complex federation                 │
│  └─ Use when: Already invested in AWS ecosystem                     │
│                                                                     │
│  Auth0                                                              │
│  ├─ Best for: Developer-friendly, custom requirements               │
│  ├─ Pros: Extensive customization, excellent docs                   │
│  ├─ Cons: Can get expensive at scale                                │
│  └─ Use when: Need flexibility and rapid development                │
│                                                                     │
│  Microsoft Entra ID (formerly Azure AD)                             │
│  ├─ Best for: Microsoft/O365 enterprises                            │
│  ├─ Pros: SSO with Microsoft apps, enterprise features              │
│  ├─ Cons: Complex setup, Microsoft-centric                          │
│  └─ Use when: Enterprise already uses Microsoft 365                 │
│                                                                     │
│  Okta                                                               │
│  ├─ Best for: Large enterprises, workforce identity                 │
│  ├─ Pros: Enterprise features, SSO across apps                      │
│  ├─ Cons: Expensive, complex                                        │
│  └─ Use when: Enterprise-grade requirements                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Common Integration Pattern

Regardless of the IdP, the integration pattern is similar:

```rust
use crate::auth::{JwtValidator, JwtValidatorConfig};

pub enum IdentityProvider {
    Cognito {
        region: String,
        user_pool_id: String,
        client_id: String,
    },
    Auth0 {
        domain: String,
        audience: String,
    },
    Entra {
        tenant_id: String,
        client_id: String,
    },
}

impl IdentityProvider {
    pub fn into_validator(self) -> JwtValidator {
        let config = match self {
            IdentityProvider::Cognito { region, user_pool_id, client_id } => {
                JwtValidatorConfig::cognito(&region, &user_pool_id, &client_id)
            }
            IdentityProvider::Auth0 { domain, audience } => {
                JwtValidatorConfig::auth0(&domain, &audience)
            }
            IdentityProvider::Entra { tenant_id, client_id } => {
                JwtValidatorConfig::entra(&tenant_id, &client_id)
            }
        };

        JwtValidator::new(config)
    }
}
```

## Configuration from Environment

Load IdP configuration from environment variables:

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct IdpConfig {
    pub provider: IdentityProvider,
}

impl IdpConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let provider_type = env::var("IDP_PROVIDER")
            .unwrap_or_else(|_| "cognito".to_string());

        let provider = match provider_type.as_str() {
            "cognito" => IdentityProvider::Cognito {
                region: env::var("AWS_REGION")
                    .map_err(|_| ConfigError::Missing("AWS_REGION"))?,
                user_pool_id: env::var("COGNITO_USER_POOL_ID")
                    .map_err(|_| ConfigError::Missing("COGNITO_USER_POOL_ID"))?,
                client_id: env::var("COGNITO_CLIENT_ID")
                    .map_err(|_| ConfigError::Missing("COGNITO_CLIENT_ID"))?,
            },
            "auth0" => IdentityProvider::Auth0 {
                domain: env::var("AUTH0_DOMAIN")
                    .map_err(|_| ConfigError::Missing("AUTH0_DOMAIN"))?,
                audience: env::var("AUTH0_AUDIENCE")
                    .map_err(|_| ConfigError::Missing("AUTH0_AUDIENCE"))?,
            },
            "entra" | "azure" => IdentityProvider::Entra {
                tenant_id: env::var("ENTRA_TENANT_ID")
                    .map_err(|_| ConfigError::Missing("ENTRA_TENANT_ID"))?,
                client_id: env::var("ENTRA_CLIENT_ID")
                    .map_err(|_| ConfigError::Missing("ENTRA_CLIENT_ID"))?,
            },
            _ => return Err(ConfigError::InvalidProvider(provider_type)),
        };

        Ok(Self { provider })
    }
}
```

## Provider-Specific Claim Mapping

Each IdP structures claims differently:

```rust
#[derive(Debug)]
pub struct UserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
    pub scopes: Vec<String>,
}

impl UserInfo {
    /// Parse claims based on IdP format
    pub fn from_claims(claims: &Claims, provider: &IdentityProvider) -> Self {
        match provider {
            IdentityProvider::Cognito { .. } => Self::from_cognito(claims),
            IdentityProvider::Auth0 { .. } => Self::from_auth0(claims),
            IdentityProvider::Entra { .. } => Self::from_entra(claims),
        }
    }

    fn from_cognito(claims: &Claims) -> Self {
        // Cognito uses:
        // - sub: user ID (UUID)
        // - email: user email
        // - cognito:username: username
        // - cognito:groups: array of group names
        Self {
            id: claims.sub.clone(),
            email: claims.email.clone(),
            name: claims.get("cognito:username").cloned(),
            groups: claims.get_array("cognito:groups").unwrap_or_default(),
            scopes: claims.scope_list(),
        }
    }

    fn from_auth0(claims: &Claims) -> Self {
        // Auth0 uses:
        // - sub: provider|user_id (e.g., "auth0|123" or "google-oauth2|456")
        // - email: user email
        // - name: display name
        // - permissions: array of permission strings
        Self {
            id: claims.sub.clone(),
            email: claims.email.clone(),
            name: claims.name.clone(),
            groups: claims.get_array("https://yourapp/groups").unwrap_or_default(),
            scopes: claims.permissions.clone().unwrap_or_else(|| claims.scope_list()),
        }
    }

    fn from_entra(claims: &Claims) -> Self {
        // Entra ID uses:
        // - oid: object ID (GUID)
        // - preferred_username: UPN (user@domain.com)
        // - name: display name
        // - groups: array of group GUIDs
        // - roles: array of app role names
        Self {
            id: claims.get("oid").unwrap_or(&claims.sub).clone(),
            email: claims.get("preferred_username").cloned(),
            name: claims.name.clone(),
            groups: claims.get_array("groups").unwrap_or_default(),
            scopes: claims.get_array("roles").unwrap_or_else(|| claims.scope_list()),
        }
    }
}
```

## Federation Patterns

### Enterprise Federation

Many enterprises federate to their corporate IdP:

```
┌─────────────────────────────────────────────────────────────────────┐
│                  Corporate Federation                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  User → Corporate IdP (Okta/Entra) → OAuth Provider → MCP Server    │
│                                                                     │
│  1. User clicks "Login with Corporate SSO"                          │
│  2. Redirected to corporate IdP (Okta, Entra, etc.)                 │
│  3. User authenticates with corporate credentials                   │
│  4. Corporate IdP issues SAML assertion to OAuth provider           │
│  5. OAuth provider (Cognito/Auth0) issues JWT                       │
│  6. MCP server validates JWT                                        │
│                                                                     │
│  Benefits:                                                          │
│  • Single sign-on across all apps                                   │
│  • Central user management                                          │
│  • Automatic deprovisioning when employees leave                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Social Login Federation

For consumer applications:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Social Login Federation                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  User → Social Provider (Google/GitHub) → OAuth Provider → MCP      │
│                                                                     │
│  Cognito: Social identity pools                                     │
│  Auth0: Social connections                                          │
│  Entra: External identities                                         │
│                                                                     │
│  User identity format varies:                                       │
│  • Cognito: "us-east-1:abc123-def456"                               │
│  • Auth0: "google-oauth2|1234567890"                                │
│  • Entra: "external_identity_guid"                                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Scope Design by Provider

### Cognito Scopes

```bash
# Cognito uses OAuth scopes + custom scopes from resource servers
# Define custom scopes in Cognito resource server:

aws cognito-idp create-resource-server \
  --user-pool-id us-east-1_xxxx \
  --identifier mcp-server \
  --name "MCP Server" \
  --scopes ScopeName=read:tools,ScopeDescription="Read tools" \
          ScopeName=execute:tools,ScopeDescription="Execute tools"
```

```rust
// Cognito scope format: "resource-server/scope"
fn cognito_scopes(claims: &Claims) -> Vec<String> {
    claims.scope
        .as_ref()
        .map(|s| {
            s.split_whitespace()
                .filter_map(|scope| {
                    // Strip resource server prefix if present
                    scope.split('/').last().map(String::from)
                })
                .collect()
        })
        .unwrap_or_default()
}
```

### Auth0 Permissions

```rust
// Auth0 uses permissions array (from RBAC)
fn auth0_permissions(claims: &Claims) -> Vec<String> {
    // Prefer permissions if available (RBAC)
    if let Some(perms) = &claims.permissions {
        return perms.clone();
    }
    // Fall back to scope string
    claims.scope_list()
}
```

### Entra App Roles

```rust
// Entra ID uses app roles (defined in app registration)
fn entra_roles(claims: &Claims) -> Vec<String> {
    claims.get_array("roles").unwrap_or_default()
}
```

## Testing with Each Provider

### Development Tokens

Each provider has ways to get test tokens:

```bash
# Cognito: Use AWS CLI
aws cognito-idp admin-initiate-auth \
  --user-pool-id us-east-1_xxxx \
  --client-id your-client-id \
  --auth-flow ADMIN_USER_PASSWORD_AUTH \
  --auth-parameters USERNAME=testuser,PASSWORD=TestPass123!

# Auth0: Use Management API or test application
curl --request POST \
  --url 'https://your-tenant.auth0.com/oauth/token' \
  --header 'content-type: application/x-www-form-urlencoded' \
  --data grant_type=password \
  --data username=testuser@example.com \
  --data 'password=TestPass123!' \
  --data client_id=your-client-id \
  --data client_secret=your-client-secret

# Entra: Use Azure CLI
az account get-access-token --resource your-client-id
```

### Mock Validator for Tests

```rust
#[cfg(test)]
pub struct MockValidator {
    user_id: String,
    scopes: Vec<String>,
}

#[cfg(test)]
impl MockValidator {
    pub fn user(id: &str) -> Self {
        Self {
            user_id: id.to_string(),
            scopes: vec!["read:tools".into()],
        }
    }

    pub fn admin(id: &str) -> Self {
        Self {
            user_id: id.to_string(),
            scopes: vec!["admin:*".into()],
        }
    }

    pub fn with_scopes(mut self, scopes: &[&str]) -> Self {
        self.scopes = scopes.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn into_context(self) -> AuthContext {
        AuthContext {
            user_id: self.user_id,
            email: Some("test@example.com".into()),
            name: Some("Test User".into()),
            scopes: self.scopes.into_iter().collect(),
        }
    }
}
```

## Security Considerations

### Token Audience Validation

Each provider sets audience differently:

| Provider | Audience Value |
|----------|---------------|
| Cognito | Client ID |
| Auth0 | API identifier (custom URL) |
| Entra | Client ID or Application ID URI |

```rust
// Always validate audience matches your configuration
if !claims.aud.contains(&self.config.audience) {
    return Err(AuthError::ValidationFailed("Invalid audience"));
}
```

### Issuer Validation

```rust
// Expected issuers
let cognito_iss = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxx";
let auth0_iss = "https://your-tenant.auth0.com/";
let entra_iss = "https://login.microsoftonline.com/tenant-id/v2.0";

// Validate issuer exactly matches
if claims.iss != expected_issuer {
    return Err(AuthError::ValidationFailed("Invalid issuer"));
}
```

## Chapter Overview

The following sections provide detailed setup guides for each provider:

1. [AWS Cognito](./ch14-01-cognito.md) - User pools, federation, and AWS integration
2. [Auth0](./ch14-02-auth0.md) - Applications, APIs, and custom rules
3. [Microsoft Entra ID](./ch14-03-entra.md) - App registrations and enterprise features
4. [Multi-Tenant Considerations](./ch14-04-multitenant.md) - Supporting multiple organizations

## Summary

**The most important takeaway: Use what you already have.** If your organization uses Okta, use Okta. If you're a Microsoft shop, use Entra ID. If you're all-in on AWS, use Cognito. Don't introduce a new identity provider just for MCP servers.

The providers in this chapter are **examples**, not recommendations. The patterns work with any OIDC-compliant provider:

1. **Configuration** - Every provider needs: issuer URL, audience, JWKS URI
2. **Claim mapping** - Providers structure user info differently (adapt `from_claims`)
3. **Scope handling** - Some use scopes, some use permissions, some use roles
4. **Testing** - Each provider has ways to get development tokens

**If your provider isn't covered here:** That's fine. You need four things:
1. The JWKS URI (usually `/.well-known/jwks.json`)
2. The issuer URL (for token validation)
3. Your app's audience value
4. Understanding of how claims are structured

The code patterns in this chapter translate directly to any provider.

---

*Continue to [AWS Cognito](./ch14-01-cognito.md) →*
