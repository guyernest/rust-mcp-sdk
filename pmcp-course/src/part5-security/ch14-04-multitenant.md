# Multi-Tenant Considerations

Multi-tenant MCP servers serve multiple organizations from a single deployment. This chapter covers architecture patterns, isolation strategies, and security considerations for multi-tenant deployments.

## Do You Need Multi-Tenancy?

**Most organizations don't.** Before diving into multi-tenant complexity, consider whether you actually need it:

| Scenario | Multi-Tenant? | Why |
|----------|---------------|-----|
| **Internal MCP server** for your company | No | Single organization, use your IdP directly |
| **Department-specific** servers | No | Deploy separate servers per department |
| **SaaS product** serving multiple customers | **Yes** | Multiple organizations, shared infrastructure |
| **Partner integrations** with isolated data | **Yes** | Multiple external organizations |
| **Enterprise platform** with subsidiaries | Maybe | Could use separate deployments or multi-tenant |

**The rule of thumb:** If all your users come from the same organization (even with different teams or roles), you don't need multi-tenancy. Your IdP handles groups and permissions within the organization.

Multi-tenancy adds significant complexity:
- Tenant isolation at every layer (code, data, rate limits)
- Cross-tenant attack surface to protect
- Tenant provisioning and lifecycle management
- Complex debugging (which tenant had the issue?)

Only adopt it if you're building a shared platform for multiple organizations.

## The Easy Way: `cargo pmcp` Multi-Tenant Mode

If you do need multi-tenancy, `cargo pmcp` provides configuration support:

```bash
# Initialize with multi-tenant support
cargo pmcp deploy init --target pmcp-run --oauth auth0 --multi-tenant

# This creates/updates .pmcp/deploy.toml with:
```

```toml
# .pmcp/deploy.toml
[auth]
enabled = true
provider = "auth0"  # Or cognito, entra—any provider works
domain = "your-tenant.auth0.com"

[auth.multi_tenant]
enabled = true
# How to identify the tenant from the JWT
tenant_claim = "org_id"  # Auth0 Organizations
# Or: "tid" for Entra ID
# Or: "custom:tenant_id" for Cognito

# Tenant isolation strategy
isolation = "row_level_security"  # Or "schema_per_tenant", "prefix"

# Default rate limit per tenant (requests per minute)
default_rate_limit = 100
```

### What Multi-Tenant Mode Enables

When you deploy with multi-tenant enabled:

1. **Tenant extraction middleware** - Automatically extracts tenant ID from JWT claims
2. **Tenant context injection** - Every tool receives `TenantContext` in its context
3. **Database isolation** - Configures RLS policies or schema-per-tenant
4. **Rate limiting** - Per-tenant rate limits to prevent noisy neighbors
5. **Audit logging** - All operations tagged with tenant ID

Your tools receive the tenant automatically:

```rust
pub async fn run(
    &self,
    input: Input,
    context: &ToolContext,
) -> Result<Output> {
    // Tenant is extracted from JWT by middleware
    let tenant = context.tenant()?;  // TenantContext

    // All database operations automatically scoped
    let data = self.db.query(&tenant, "SELECT * FROM resources").await?;

    Ok(Output { data })
}
```

## Manual Setup (For Complex Requirements)

If you need custom tenant resolution, complex isolation patterns, or cross-tenant admin operations, configure multi-tenancy manually. The rest of this chapter covers these advanced patterns.

## Multi-Tenant Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                Multi-Tenant MCP Architecture                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Organization A          Organization B          Organization C     │
│  ┌─────────────┐        ┌─────────────┐        ┌─────────────┐      │
│  │ MCP Client  │        │ MCP Client  │        │ MCP Client  │      │
│  └──────┬──────┘        └──────┬──────┘        └──────┬──────┘      │
│         │                      │                      │             │
│         │ JWT (tenant_a)       │ JWT (tenant_b)       │ JWT (c)     │
│         │                      │                      │             │
│         └──────────────────────┼──────────────────────┘             │
│                                │                                    │
│                                ▼                                    │
│                    ┌───────────────────────┐                        │
│                    │    MCP Server         │                        │
│                    │    ───────────        │                        │
│                    │    • Extract tenant   │                        │
│                    │    • Validate access  │                        │
│                    │    • Isolate data     │                        │
│                    └───────────┬───────────┘                        │
│                                │                                    │
│         ┌──────────────────────┼──────────────────────┐             │
│         │                      │                      │             │
│         ▼                      ▼                      ▼             │
│  ┌─────────────┐        ┌─────────────┐        ┌─────────────┐      │
│  │ Tenant A    │        │ Tenant B    │        │ Tenant C    │      │
│  │ Data/Config │        │ Data/Config │        │ Data/Config │      │
│  └─────────────┘        └─────────────┘        └─────────────┘      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Tenant Identification

### From JWT Claims

Each identity provider signals tenant differently:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub tenant_name: Option<String>,
    pub user_id: String,
    pub scopes: Vec<String>,
}

impl TenantContext {
    /// Extract tenant from Cognito claims
    pub fn from_cognito(claims: &CognitoClaims) -> Result<Self, TenantError> {
        // Cognito: Use custom attribute or user pool ID
        let tenant_id = claims
            .get_custom("tenant_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or(TenantError::MissingTenant)?;

        Ok(Self {
            tenant_id,
            tenant_name: claims.get_custom("tenant_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            user_id: claims.sub.clone(),
            scopes: claims.scope_list(),
        })
    }

    /// Extract tenant from Auth0 claims
    pub fn from_auth0(claims: &Auth0Claims) -> Result<Self, TenantError> {
        // Auth0: Use organization claim
        let tenant_id = claims
            .custom.get("org_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or(TenantError::MissingTenant)?;

        Ok(Self {
            tenant_id,
            tenant_name: claims.custom.get("org_name")
                .and_then(|v| v.as_str())
                .map(String::from),
            user_id: claims.sub.clone(),
            scopes: claims.permissions_list(),
        })
    }

    /// Extract tenant from Entra ID claims
    pub fn from_entra(claims: &EntraClaims) -> Result<Self, TenantError> {
        // Entra: Use tid (tenant ID) claim
        Ok(Self {
            tenant_id: claims.tid.clone(),
            tenant_name: None, // Can be fetched from Graph API
            user_id: claims.oid.clone(),
            scopes: claims.roles.clone().unwrap_or_default(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TenantError {
    #[error("Missing tenant identifier in token")]
    MissingTenant,

    #[error("Unknown tenant: {0}")]
    UnknownTenant(String),

    #[error("Tenant access denied: {0}")]
    AccessDenied(String),
}
```

### Tenant Registry

Validate and enrich tenant information:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct TenantInfo {
    pub id: String,
    pub name: String,
    pub config: TenantConfig,
    pub status: TenantStatus,
}

#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub database_schema: String,
    pub storage_prefix: String,
    pub rate_limit: u32,
    pub allowed_tools: Vec<String>,
    pub feature_flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TenantStatus {
    Active,
    Suspended,
    Trial { expires_at: u64 },
}

pub struct TenantRegistry {
    tenants: Arc<RwLock<HashMap<String, TenantInfo>>>,
}

impl TenantRegistry {
    pub async fn get(&self, tenant_id: &str) -> Result<TenantInfo, TenantError> {
        let tenants = self.tenants.read().await;

        let tenant = tenants.get(tenant_id)
            .ok_or_else(|| TenantError::UnknownTenant(tenant_id.to_string()))?;

        // Check tenant status
        match &tenant.status {
            TenantStatus::Active => Ok(tenant.clone()),
            TenantStatus::Suspended => {
                Err(TenantError::AccessDenied("Tenant suspended".into()))
            }
            TenantStatus::Trial { expires_at } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now > *expires_at {
                    Err(TenantError::AccessDenied("Trial expired".into()))
                } else {
                    Ok(tenant.clone())
                }
            }
        }
    }

    pub async fn refresh(&self) -> Result<(), TenantError> {
        // Load tenants from database or config service
        // This should be called periodically or on cache miss
        todo!("Load tenants from persistent storage")
    }
}
```

## Data Isolation Strategies

### Strategy 1: Schema-Per-Tenant

Each tenant gets a separate database schema:

```rust
use sqlx::{Pool, Postgres};

pub struct SchemaIsolatedDb {
    pool: Pool<Postgres>,
}

impl SchemaIsolatedDb {
    /// Execute query in tenant's schema
    pub async fn query_tenant<T>(
        &self,
        tenant: &TenantContext,
        query: &str,
    ) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        // Set search_path to tenant's schema
        let schema = self.tenant_schema(&tenant.tenant_id);

        sqlx::query(&format!("SET search_path TO {}", schema))
            .execute(&self.pool)
            .await?;

        let results = sqlx::query_as::<_, T>(query)
            .fetch_all(&self.pool)
            .await?;

        // Reset to public schema
        sqlx::query("SET search_path TO public")
            .execute(&self.pool)
            .await?;

        Ok(results)
    }

    fn tenant_schema(&self, tenant_id: &str) -> String {
        // Sanitize tenant_id to prevent SQL injection
        let safe_id = tenant_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();

        format!("tenant_{}", safe_id)
    }

    /// Create schema for new tenant
    pub async fn provision_tenant(&self, tenant_id: &str) -> Result<(), DbError> {
        let schema = self.tenant_schema(tenant_id);

        // Create schema
        sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {}", schema))
            .execute(&self.pool)
            .await?;

        // Run migrations in tenant schema
        sqlx::query(&format!("SET search_path TO {}", schema))
            .execute(&self.pool)
            .await?;

        // Create tables...
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS resources (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name TEXT NOT NULL,
                content JSONB,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
        "#)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
```

### Strategy 2: Row-Level Security

Use database row-level security for shared tables:

```sql
-- PostgreSQL RLS setup
CREATE TABLE resources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id TEXT NOT NULL,
    name TEXT NOT NULL,
    content JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create index for tenant filtering
CREATE INDEX idx_resources_tenant ON resources(tenant_id);

-- Enable RLS
ALTER TABLE resources ENABLE ROW LEVEL SECURITY;

-- Policy: Users can only see their tenant's data
CREATE POLICY tenant_isolation ON resources
    FOR ALL
    USING (tenant_id = current_setting('app.tenant_id'));

-- Force RLS for all users except superusers
ALTER TABLE resources FORCE ROW LEVEL SECURITY;
```

```rust
pub struct RlsIsolatedDb {
    pool: Pool<Postgres>,
}

impl RlsIsolatedDb {
    /// Execute query with tenant context
    pub async fn with_tenant<F, T>(
        &self,
        tenant: &TenantContext,
        operation: F,
    ) -> Result<T, DbError>
    where
        F: FnOnce(&Pool<Postgres>) -> futures::future::BoxFuture<'_, Result<T, DbError>>,
    {
        // Set tenant context for RLS
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(&tenant.tenant_id)
            .execute(&self.pool)
            .await?;

        // Execute the operation
        operation(&self.pool).await
    }

    /// Query resources (automatically filtered by RLS)
    pub async fn list_resources(
        &self,
        tenant: &TenantContext,
    ) -> Result<Vec<Resource>, DbError> {
        self.with_tenant(tenant, |pool| {
            Box::pin(async move {
                sqlx::query_as::<_, Resource>("SELECT * FROM resources")
                    .fetch_all(pool)
                    .await
                    .map_err(DbError::from)
            })
        }).await
    }
}
```

### Strategy 3: Prefix-Based Isolation

For key-value stores and object storage:

```rust
pub struct PrefixIsolatedStorage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl PrefixIsolatedStorage {
    /// Get object with tenant prefix
    pub async fn get(
        &self,
        tenant: &TenantContext,
        key: &str,
    ) -> Result<Vec<u8>, StorageError> {
        let prefixed_key = self.tenant_key(tenant, key);

        let response = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&prefixed_key)
            .send()
            .await?;

        let bytes = response.body.collect().await?.into_bytes();
        Ok(bytes.to_vec())
    }

    /// Put object with tenant prefix
    pub async fn put(
        &self,
        tenant: &TenantContext,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), StorageError> {
        let prefixed_key = self.tenant_key(tenant, key);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&prefixed_key)
            .body(data.into())
            .send()
            .await?;

        Ok(())
    }

    /// List objects for tenant
    pub async fn list(
        &self,
        tenant: &TenantContext,
        prefix: &str,
    ) -> Result<Vec<String>, StorageError> {
        let full_prefix = self.tenant_key(tenant, prefix);

        let response = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&full_prefix)
            .send()
            .await?;

        let keys = response.contents()
            .iter()
            .filter_map(|obj| obj.key())
            .map(|k| k.strip_prefix(&format!("{}/", tenant.tenant_id))
                .unwrap_or(k)
                .to_string())
            .collect();

        Ok(keys)
    }

    fn tenant_key(&self, tenant: &TenantContext, key: &str) -> String {
        format!("{}/{}", tenant.tenant_id, key)
    }
}
```

## Tenant-Aware Tools

### Tool with Tenant Context

```rust
use mcp_server::{Tool, ToolContext, ToolError};

pub struct ListDocumentsTool {
    storage: Arc<PrefixIsolatedStorage>,
}

impl Tool for ListDocumentsTool {
    type Input = ListDocumentsInput;
    type Output = ListDocumentsOutput;

    fn name(&self) -> &str {
        "list_documents"
    }

    async fn run(
        &self,
        input: Self::Input,
        context: &ToolContext,
    ) -> Result<Self::Output, ToolError> {
        // Get tenant from context (extracted from JWT by middleware)
        let tenant = context.tenant()
            .ok_or_else(|| ToolError::Unauthorized("Missing tenant context"))?;

        // Operation is automatically scoped to tenant
        let documents = self.storage
            .list(&tenant, &input.prefix.unwrap_or_default())
            .await
            .map_err(|e| ToolError::Internal(e.to_string()))?;

        Ok(ListDocumentsOutput { documents })
    }
}
```

### Tenant-Specific Tool Configuration

```rust
pub struct TenantAwareToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    tenant_registry: Arc<TenantRegistry>,
}

impl TenantAwareToolRegistry {
    /// List tools available to tenant
    pub async fn list_for_tenant(
        &self,
        tenant: &TenantContext,
    ) -> Result<Vec<ToolInfo>, ToolError> {
        let tenant_info = self.tenant_registry
            .get(&tenant.tenant_id)
            .await
            .map_err(|e| ToolError::Unauthorized(e.to_string()))?;

        // Filter tools based on tenant's allowed list
        let available: Vec<_> = self.tools
            .iter()
            .filter(|(name, _)| {
                tenant_info.config.allowed_tools.is_empty() ||
                tenant_info.config.allowed_tools.contains(*name)
            })
            .map(|(name, tool)| ToolInfo {
                name: name.clone(),
                description: tool.description().to_string(),
            })
            .collect();

        Ok(available)
    }

    /// Check if tenant can use tool
    pub async fn can_use(
        &self,
        tenant: &TenantContext,
        tool_name: &str,
    ) -> Result<bool, ToolError> {
        let tenant_info = self.tenant_registry
            .get(&tenant.tenant_id)
            .await
            .map_err(|e| ToolError::Unauthorized(e.to_string()))?;

        // Empty allowed_tools means all tools are allowed
        if tenant_info.config.allowed_tools.is_empty() {
            return Ok(true);
        }

        Ok(tenant_info.config.allowed_tools.contains(&tool_name.to_string()))
    }
}
```

## Rate Limiting Per Tenant

```rust
use std::time::{Duration, Instant};
use dashmap::DashMap;

pub struct TenantRateLimiter {
    limits: DashMap<String, RateLimitState>,
    default_limit: u32,
}

struct RateLimitState {
    tokens: u32,
    last_refill: Instant,
    limit: u32,
}

impl TenantRateLimiter {
    pub fn new(default_limit: u32) -> Self {
        Self {
            limits: DashMap::new(),
            default_limit,
        }
    }

    /// Check and consume rate limit
    pub async fn check(
        &self,
        tenant: &TenantContext,
        tenant_info: &TenantInfo,
    ) -> Result<(), RateLimitError> {
        let limit = if tenant_info.config.rate_limit > 0 {
            tenant_info.config.rate_limit
        } else {
            self.default_limit
        };

        let mut state = self.limits
            .entry(tenant.tenant_id.clone())
            .or_insert_with(|| RateLimitState {
                tokens: limit,
                last_refill: Instant::now(),
                limit,
            });

        // Refill tokens (1 per second)
        let elapsed = state.last_refill.elapsed();
        let refill = elapsed.as_secs() as u32;
        if refill > 0 {
            state.tokens = (state.tokens + refill).min(state.limit);
            state.last_refill = Instant::now();
        }

        // Consume token
        if state.tokens > 0 {
            state.tokens -= 1;
            Ok(())
        } else {
            Err(RateLimitError::Exceeded {
                tenant_id: tenant.tenant_id.clone(),
                retry_after: Duration::from_secs(1),
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for tenant {tenant_id}, retry after {retry_after:?}")]
    Exceeded {
        tenant_id: String,
        retry_after: Duration,
    },
}
```

## Multi-Tenant Middleware

### Axum Middleware

```rust
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

pub async fn tenant_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Extract auth context (set by auth middleware)
    let auth = request.extensions()
        .get::<AuthContext>()
        .ok_or(ApiError::Unauthorized)?;

    // Extract tenant from claims
    let tenant_context = match &state.idp_type {
        IdpType::Cognito => TenantContext::from_cognito(&auth.claims)?,
        IdpType::Auth0 => TenantContext::from_auth0(&auth.claims)?,
        IdpType::Entra => TenantContext::from_entra(&auth.claims)?,
    };

    // Validate tenant
    let tenant_info = state.tenant_registry
        .get(&tenant_context.tenant_id)
        .await?;

    // Check rate limit
    state.rate_limiter
        .check(&tenant_context, &tenant_info)
        .await?;

    // Add tenant context to request
    request.extensions_mut().insert(tenant_context);
    request.extensions_mut().insert(tenant_info);

    Ok(next.run(request).await)
}
```

### Request Context Extraction

```rust
use axum::extract::FromRequestParts;

pub struct Tenant(pub TenantContext);

#[async_trait]
impl<S> FromRequestParts<S> for Tenant
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts.extensions
            .get::<TenantContext>()
            .cloned()
            .map(Tenant)
            .ok_or(ApiError::MissingTenantContext)
    }
}

// Use in handlers
async fn list_resources(
    Tenant(tenant): Tenant,
    State(state): State<AppState>,
) -> Result<Json<Vec<Resource>>, ApiError> {
    let resources = state.db.list_resources(&tenant).await?;
    Ok(Json(resources))
}
```

## Cross-Tenant Operations

### Admin Access Pattern

```rust
#[derive(Debug, Clone)]
pub enum TenantScope {
    /// Operations scoped to a single tenant
    Single(TenantContext),

    /// Admin operations across all tenants
    Global { admin_id: String },
}

impl TenantScope {
    pub fn from_auth(auth: &AuthContext) -> Result<Self, TenantError> {
        // Check for global admin role
        if auth.has_scope("admin:global") {
            return Ok(TenantScope::Global {
                admin_id: auth.user_id.clone(),
            });
        }

        // Extract tenant for normal users
        let tenant = TenantContext::from_claims(&auth.claims)?;
        Ok(TenantScope::Single(tenant))
    }
}

pub struct AdminTool {
    db: Arc<RlsIsolatedDb>,
}

impl AdminTool {
    pub async fn list_all_tenants(
        &self,
        scope: &TenantScope,
    ) -> Result<Vec<TenantInfo>, ToolError> {
        match scope {
            TenantScope::Global { admin_id } => {
                tracing::info!(admin = %admin_id, "Listing all tenants");
                // Query without tenant filter
                self.db.list_all_tenants().await
            }
            TenantScope::Single(_) => {
                Err(ToolError::Forbidden("Global admin access required"))
            }
        }
    }

    pub async fn impersonate_tenant(
        &self,
        scope: &TenantScope,
        target_tenant_id: &str,
    ) -> Result<TenantContext, ToolError> {
        match scope {
            TenantScope::Global { admin_id } => {
                tracing::warn!(
                    admin = %admin_id,
                    tenant = %target_tenant_id,
                    "Admin impersonating tenant"
                );

                // Create impersonated context
                Ok(TenantContext {
                    tenant_id: target_tenant_id.to_string(),
                    tenant_name: None,
                    user_id: format!("admin:{}", admin_id),
                    scopes: vec!["admin:impersonate".into()],
                })
            }
            TenantScope::Single(_) => {
                Err(ToolError::Forbidden("Cannot impersonate other tenants"))
            }
        }
    }
}
```

## Tenant Provisioning

### Automated Provisioning

```rust
pub struct TenantProvisioner {
    db: Arc<SchemaIsolatedDb>,
    storage: Arc<PrefixIsolatedStorage>,
    registry: Arc<TenantRegistry>,
}

impl TenantProvisioner {
    pub async fn provision(&self, request: ProvisionRequest) -> Result<TenantInfo, ProvisionError> {
        let tenant_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(tenant_id = %tenant_id, "Provisioning new tenant");

        // 1. Create database schema
        self.db.provision_tenant(&tenant_id).await?;

        // 2. Create storage prefix (just needs first write)
        self.storage.put(
            &TenantContext {
                tenant_id: tenant_id.clone(),
                tenant_name: Some(request.name.clone()),
                user_id: "system".into(),
                scopes: vec![],
            },
            ".tenant-marker",
            b"initialized".to_vec(),
        ).await?;

        // 3. Create tenant record
        let tenant_info = TenantInfo {
            id: tenant_id.clone(),
            name: request.name,
            config: TenantConfig {
                database_schema: format!("tenant_{}", tenant_id),
                storage_prefix: tenant_id.clone(),
                rate_limit: request.rate_limit.unwrap_or(100),
                allowed_tools: request.allowed_tools,
                feature_flags: request.feature_flags,
            },
            status: if request.trial_days > 0 {
                let expires_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() + (request.trial_days as u64 * 86400);
                TenantStatus::Trial { expires_at }
            } else {
                TenantStatus::Active
            },
        };

        // 4. Register tenant
        self.registry.add(tenant_info.clone()).await?;

        tracing::info!(tenant_id = %tenant_id, "Tenant provisioned successfully");

        Ok(tenant_info)
    }

    pub async fn deprovision(&self, tenant_id: &str) -> Result<(), ProvisionError> {
        tracing::warn!(tenant_id = %tenant_id, "Deprovisioning tenant");

        // 1. Mark tenant as suspended first
        self.registry.update_status(tenant_id, TenantStatus::Suspended).await?;

        // 2. Archive data (don't delete immediately)
        // ... archive to cold storage ...

        // 3. Drop schema after retention period
        // ... scheduled job ...

        Ok(())
    }
}

pub struct ProvisionRequest {
    pub name: String,
    pub rate_limit: Option<u32>,
    pub allowed_tools: Vec<String>,
    pub feature_flags: HashMap<String, bool>,
    pub trial_days: u32,
}
```

## Testing Multi-Tenant Systems

### Test Helpers

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_tenant(id: &str) -> TenantContext {
        TenantContext {
            tenant_id: id.to_string(),
            tenant_name: Some(format!("Test Tenant {}", id)),
            user_id: format!("user-{}", id),
            scopes: vec!["read:tools".into(), "execute:tools".into()],
        }
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let storage = setup_test_storage().await;

        let tenant_a = test_tenant("tenant-a");
        let tenant_b = test_tenant("tenant-b");

        // Write data for tenant A
        storage.put(&tenant_a, "secret.txt", b"tenant-a-secret".to_vec())
            .await.unwrap();

        // Write data for tenant B
        storage.put(&tenant_b, "secret.txt", b"tenant-b-secret".to_vec())
            .await.unwrap();

        // Tenant A can only see their data
        let data_a = storage.get(&tenant_a, "secret.txt").await.unwrap();
        assert_eq!(data_a, b"tenant-a-secret");

        // Tenant B can only see their data
        let data_b = storage.get(&tenant_b, "secret.txt").await.unwrap();
        assert_eq!(data_b, b"tenant-b-secret");

        // Tenant A cannot access tenant B's data
        let list_a = storage.list(&tenant_a, "").await.unwrap();
        assert!(!list_a.iter().any(|k| k.contains("tenant-b")));
    }

    #[tokio::test]
    async fn test_cross_tenant_blocked() {
        let db = setup_test_db().await;

        let tenant_a = test_tenant("tenant-a");
        let tenant_b = test_tenant("tenant-b");

        // Create resource for tenant A
        db.with_tenant(&tenant_a, |pool| {
            Box::pin(async move {
                sqlx::query("INSERT INTO resources (name) VALUES ('secret')")
                    .execute(pool)
                    .await
                    .map_err(DbError::from)
            })
        }).await.unwrap();

        // Tenant B should not see tenant A's resource
        let resources = db.list_resources(&tenant_b).await.unwrap();
        assert!(resources.is_empty());
    }
}
```

### Integration Test Setup

```rust
#[cfg(test)]
pub struct MultiTenantTestHarness {
    server: TestServer,
    tenants: Vec<(TenantContext, String)>, // (context, token)
}

#[cfg(test)]
impl MultiTenantTestHarness {
    pub async fn setup(num_tenants: usize) -> Self {
        let server = TestServer::new().await;

        let mut tenants = Vec::new();
        for i in 0..num_tenants {
            let tenant_id = format!("test-tenant-{}", i);
            let context = TenantContext {
                tenant_id: tenant_id.clone(),
                tenant_name: Some(format!("Test Tenant {}", i)),
                user_id: format!("user-{}", i),
                scopes: vec!["read:tools".into(), "execute:tools".into()],
            };

            // Generate test token for tenant
            let token = generate_test_token(&context);
            tenants.push((context, token));
        }

        Self { server, tenants }
    }

    pub fn tenant(&self, index: usize) -> (&TenantContext, &str) {
        let (ctx, token) = &self.tenants[index];
        (ctx, token)
    }
}
```

## Summary

**First, ask: Do you need multi-tenancy?** Most organizations don't. If all your users come from the same organization, single-tenant is simpler and more secure. Multi-tenancy is for SaaS platforms serving multiple external organizations.

**If you do need it:** Use `cargo pmcp deploy init --multi-tenant` to configure tenant extraction, isolation strategy, and per-tenant rate limiting. Your tools receive `TenantContext` automatically.

**For advanced requirements**, multi-tenant MCP servers require:

1. **Tenant Identification** - Extract tenant from JWT claims (org_id, tid, custom claims)
2. **Data Isolation** - Schema-per-tenant, row-level security, or prefix-based
3. **Tool Isolation** - Tenant-specific tool access and configuration
4. **Rate Limiting** - Per-tenant limits to prevent noisy neighbors
5. **Admin Access** - Controlled cross-tenant operations for support

Key security principles:
- **Defense in depth** - Multiple isolation layers (middleware + database + storage)
- **Fail secure** - Default deny cross-tenant access; explicit allow only
- **Audit everything** - Log all operations with tenant ID
- **Test isolation** - Verify data cannot leak between tenants (write tests!)
- **Minimize cross-tenant** - Admin operations should be rare and heavily logged

---

*← Return to [Identity Provider Integration](./ch14-providers.md)*
