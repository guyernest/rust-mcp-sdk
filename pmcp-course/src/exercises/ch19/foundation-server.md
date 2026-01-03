::: exercise
id: ch19-01-foundation-server
difficulty: advanced
time: 60 minutes
:::

Build a reusable foundation server that provides shared infrastructure for
domain-specific MCP servers. This is an architectural pattern for organizations
with multiple MCP servers.

::: objectives
thinking:
  - When foundation servers make sense (10+ servers, code duplication)
  - The boundary between infrastructure and business logic
  - Versioning and governance for shared components
doing:
  - Design the FoundationCapability trait
  - Build auth, database, and observability foundations
  - Create composable interfaces for domain servers
  - Demonstrate composition in a domain server
:::

::: discussion
- Imagine you have 10 MCP servers, each team-owned. What code is duplicated?
- At what scale does code duplication become a maintenance burden?
- What's the cost of each team implementing auth differently?
:::

## Step 1: Design the Foundation Trait

```rust
/// Trait for composable foundation capabilities
pub trait FoundationCapability: Send + Sync {
    /// Inject this foundation's capabilities into a server builder
    fn inject_into(&self, builder: ServerBuilder) -> ServerBuilder;

    /// Optional: Initialize any async resources
    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    /// Optional: Health check for this foundation
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

## Step 2: Build Auth Foundation

```rust
pub struct AuthFoundation {
    validator: JwtValidator,
    config: AuthConfig,
}

impl AuthFoundation {
    pub fn new(config: AuthConfig) -> Result<Self> {
        let validator = JwtValidator::new()
            .with_config(config.clone())?;
        Ok(Self { validator, config })
    }

    pub fn validator(&self) -> &JwtValidator {
        &self.validator
    }
}

impl FoundationCapability for AuthFoundation {
    fn inject_into(&self, builder: ServerBuilder) -> ServerBuilder {
        builder
            .middleware(OAuthMiddleware::new(self.validator.clone()))
            .tool("whoami", WhoAmITool::new())
    }
}
```

## Step 3: Build Database Foundation

```rust
pub struct DbFoundation {
    pool: Pool<Postgres>,
}

impl DbFoundation {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}

impl FoundationCapability for DbFoundation {
    fn inject_into(&self, builder: ServerBuilder) -> ServerBuilder {
        builder
            .state(self.pool.clone())
            .tool("health_check", DbHealthTool::new(self.pool.clone()))
    }

    async fn initialize(&self) -> Result<()> {
        // Verify connection on startup
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match sqlx::query("SELECT 1").execute(&self.pool).await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(e.to_string())),
        }
    }
}
```

## Step 4: Build Observability Foundation

```rust
pub struct ObservabilityFoundation {
    service_name: String,
}

impl ObservabilityFoundation {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }
}

impl FoundationCapability for ObservabilityFoundation {
    fn inject_into(&self, builder: ServerBuilder) -> ServerBuilder {
        builder
            .middleware(LoggingMiddleware::new(&self.service_name))
            .middleware(MetricsMiddleware::new(&self.service_name))
    }
}
```

## Step 5: Compose Domain Server

```rust
/// Finance domain server composes all foundations
pub async fn create_finance_server(
    auth: Arc<AuthFoundation>,
    db: Arc<DbFoundation>,
    observability: Arc<ObservabilityFoundation>,
) -> Result<Server> {
    // Initialize foundations
    db.initialize().await?;

    // Start with base builder
    let builder = ServerBuilder::new("finance-server", "1.0.0");

    // Inject foundations
    let builder = auth.inject_into(builder);
    let builder = db.inject_into(builder);
    let builder = observability.inject_into(builder);

    // Add domain-specific tools
    let builder = builder
        .tool("expense_report", ExpenseReportTool::new(db.pool()))
        .tool("invoice", InvoiceTool::new(db.pool()))
        .tool("budget_check", BudgetCheckTool::new(db.pool()));

    builder.build()
}
```

## Step 6: Main Entry Point

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::from_env()?;

    // Create shared foundations
    let auth = Arc::new(AuthFoundation::new(config.auth)?);
    let db = Arc::new(DbFoundation::new(&config.database_url).await?);
    let observability = Arc::new(ObservabilityFoundation::new("finance"));

    // Create domain server
    let server = create_finance_server(auth, db, observability).await?;

    // Run server
    server.serve("0.0.0.0:3000").await
}
```

::: hints
level_1: "Foundation = infrastructure (auth, db, logging). Domain = business (invoices, users). Keep them separated."
level_2: "Make foundations optional - domain servers should work with any combination."
level_3: "Namespace state keys (e.g., 'auth.validator', 'db.pool') to avoid conflicts."
:::

## Success Criteria

- [ ] AuthFoundation provides JWT validation middleware
- [ ] DbFoundation provides connection pool and health tool
- [ ] ObservabilityFoundation provides logging middleware
- [ ] FoundationCapability trait enables composition
- [ ] Domain server demonstrates composing all three
- [ ] Each foundation works independently

---

*This pattern scales to [Server Composition](../../part8-advanced/ch19-composition.md) for enterprise architectures.*
