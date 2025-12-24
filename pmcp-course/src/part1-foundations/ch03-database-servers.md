# Database MCP Servers

Database access is the killer app for enterprise MCP. This chapter shows how to build a production-ready database MCP server.

## The Enterprise Data Access Problem

Every enterprise has data trapped in databases:
- Customer information in CRM systems
- Financial data in ERP systems
- Analytics in data warehouses
- Operational data in PostgreSQL/MySQL

When employees need this data for AI conversations, they:
1. Request access from IT (days/weeks)
2. Learn SQL or use clunky reporting tools
3. Export to CSV
4. Copy-paste into ChatGPT

An MCP server eliminates this friction while maintaining security.

## Building db-explorer

```bash
# Add a db-explorer server to your workspace
cargo pmcp add server db-explorer --template db-explorer
```

Or create from scratch:

```bash
cargo pmcp add server sales-data --template minimal
```

### Database Connection

```rust
// src/database.rs
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use std::sync::Arc;

pub type DbPool = Arc<Pool<Sqlite>>;

pub async fn create_pool(database_url: &str) -> Result<DbPool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    Ok(Arc::new(pool))
}
```

### The Query Tool

```rust
use pmcp::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::database::DbPool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryInput {
    /// SQL query to execute (SELECT only)
    #[schemars(regex(pattern = r"^SELECT"))]
    query: String,

    /// Maximum rows to return (default: 100, max: 1000)
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 1000))]
    limit: i32,
}

fn default_limit() -> i32 { 100 }

#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryOutput {
    /// Column names
    columns: Vec<String>,
    /// Row data as JSON arrays
    rows: Vec<Vec<serde_json::Value>>,
    /// Number of rows returned
    row_count: usize,
    /// Whether more rows exist
    truncated: bool,
}

#[derive(TypedTool)]
#[tool(
    name = "query",
    description = "Execute a read-only SQL query against the database",
    annotations(read_only = true)
)]
pub struct Query {
    pool: DbPool,
}

impl Query {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn run(&self, input: QueryInput) -> Result<QueryOutput> {
        // Security: Validate query is SELECT only
        let query_upper = input.query.trim().to_uppercase();
        if !query_upper.starts_with("SELECT") {
            return Err(PmcpError::invalid_params(
                "Only SELECT queries are allowed"
            ));
        }

        // Security: Block dangerous patterns
        let dangerous_patterns = ["DROP", "DELETE", "INSERT", "UPDATE", "ALTER", "CREATE"];
        for pattern in dangerous_patterns {
            if query_upper.contains(pattern) {
                return Err(PmcpError::invalid_params(
                    format!("{} operations are not allowed", pattern)
                ));
            }
        }

        // Add LIMIT clause if not present
        let limited_query = if !query_upper.contains("LIMIT") {
            format!("{} LIMIT {}", input.query, input.limit + 1)
        } else {
            input.query.clone()
        };

        // Execute query
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(&limited_query)
            .fetch_all(self.pool.as_ref())
            .await
            .map_err(|e| PmcpError::internal(format!("Query failed: {}", e)))?;

        // Check if truncated
        let truncated = rows.len() > input.limit as usize;
        let rows: Vec<_> = rows.into_iter().take(input.limit as usize).collect();

        // Extract column names from first row
        let columns: Vec<String> = if !rows.is_empty() {
            rows[0].columns().iter().map(|c| c.name().to_string()).collect()
        } else {
            vec![]
        };

        // Convert rows to JSON
        let json_rows: Vec<Vec<serde_json::Value>> = rows
            .iter()
            .map(|row| {
                columns.iter().enumerate().map(|(i, _)| {
                    row.try_get::<serde_json::Value, _>(i)
                        .unwrap_or(serde_json::Value::Null)
                }).collect()
            })
            .collect();

        let row_count = json_rows.len();

        Ok(QueryOutput {
            columns,
            rows: json_rows,
            row_count,
            truncated,
        })
    }
}
```

### Schema Introspection

```rust
#[derive(Debug, Serialize, JsonSchema)]
pub struct TableInfo {
    name: String,
    columns: Vec<ColumnInfo>,
    row_count: Option<i64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ColumnInfo {
    name: String,
    data_type: String,
    nullable: bool,
    primary_key: bool,
}

#[derive(TypedTool)]
#[tool(
    name = "list_tables",
    description = "List all tables in the database with their schemas",
    annotations(read_only = true)
)]
pub struct ListTables {
    pool: DbPool,
}

impl ListTables {
    pub async fn run(&self, _input: ()) -> Result<Vec<TableInfo>> {
        // SQLite-specific query for table information
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut result = Vec::new();

        for (table_name,) in tables {
            // Get column info
            let columns: Vec<ColumnInfo> = sqlx::query_as::<_, (i32, String, String, bool, bool)>(
                &format!("PRAGMA table_info({})", table_name)
            )
            .fetch_all(self.pool.as_ref())
            .await?
            .into_iter()
            .map(|(_, name, data_type, not_null, pk)| ColumnInfo {
                name,
                data_type,
                nullable: !not_null,
                primary_key: pk,
            })
            .collect();

            // Get approximate row count
            let row_count: Option<i64> = sqlx::query_scalar(
                &format!("SELECT COUNT(*) FROM {}", table_name)
            )
            .fetch_one(self.pool.as_ref())
            .await
            .ok();

            result.push(TableInfo {
                name: table_name,
                columns,
                row_count,
            });
        }

        Ok(result)
    }
}
```

## SQL Safety Patterns

### Never Trust User Input

```rust
// BAD: SQL injection vulnerability
let query = format!("SELECT * FROM users WHERE name = '{}'", input.name);

// GOOD: Parameterized query
let result = sqlx::query("SELECT * FROM users WHERE name = ?")
    .bind(&input.name)
    .fetch_all(&pool)
    .await?;
```

### Allowlist, Don't Blocklist

```rust
// BAD: Trying to block dangerous patterns
if !query.contains("DROP") && !query.contains("DELETE") { ... }

// GOOD: Only allow specific patterns
let allowed_tables = ["customers", "orders", "products"];
if !allowed_tables.contains(&table_name.as_str()) {
    return Err(PmcpError::invalid_params("Table not accessible"));
}
```

### Limit Results Always

```rust
// Always add LIMIT to prevent memory exhaustion
const MAX_ROWS: i32 = 10000;

let safe_limit = input.limit.min(MAX_ROWS);
let query = format!("{} LIMIT {}", query, safe_limit);
```

## Resource-Based Data Access

For structured data, consider resources instead of SQL:

```rust
#[derive(TypedResource)]
#[resource(
    uri_template = "db://customers/{customer_id}",
    name = "customer",
    description = "Customer information by ID"
)]
pub struct CustomerResource {
    pool: DbPool,
}

impl CustomerResource {
    pub async fn read(&self, customer_id: String) -> Result<ResourceContent> {
        let customer: Customer = sqlx::query_as(
            "SELECT * FROM customers WHERE id = ?"
        )
        .bind(&customer_id)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|_| PmcpError::resource_not_found(
            format!("Customer {} not found", customer_id)
        ))?;

        Ok(ResourceContent::json(customer))
    }
}
```

## Handling Large Results

### Streaming (Advanced)

For very large results, consider streaming:

```rust
use futures::StreamExt;

pub async fn stream_query(&self, query: &str) -> impl Stream<Item = Row> {
    sqlx::query(query)
        .fetch(self.pool.as_ref())
        .map(|r| r.expect("row"))
}
```

### Pagination

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PaginatedQueryInput {
    query: String,
    #[serde(default)]
    page: i32,
    #[serde(default = "default_page_size")]
    page_size: i32,
}

fn default_page_size() -> i32 { 50 }

impl Query {
    pub async fn run_paginated(&self, input: PaginatedQueryInput) -> Result<PaginatedOutput> {
        let offset = input.page * input.page_size;
        let limited = format!(
            "{} LIMIT {} OFFSET {}",
            input.query,
            input.page_size + 1,  // +1 to detect if more pages exist
            offset
        );

        let rows = self.execute(&limited).await?;
        let has_more = rows.len() > input.page_size as usize;

        Ok(PaginatedOutput {
            rows: rows.into_iter().take(input.page_size as usize).collect(),
            page: input.page,
            has_more,
        })
    }
}
```

## Real-World Example: Sales Database

Let's build a complete sales database MCP server:

```rust
// src/main.rs
use pmcp::prelude::*;

mod database;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:sales.db".to_string());

    let pool = database::create_pool(&database_url).await?;

    // Build server with all tools
    let server = ServerBuilder::new("sales-data", "1.0.0")
        .with_description("Access sales data for analysis")
        // Query tools
        .with_tool(tools::Query::new(pool.clone()))
        .with_tool(tools::ListTables::new(pool.clone()))
        // Convenience tools
        .with_tool(tools::TopCustomers::new(pool.clone()))
        .with_tool(tools::SalesByRegion::new(pool.clone()))
        .with_tool(tools::RevenueOverTime::new(pool.clone()))
        // Resources
        .with_resource(tools::CustomerResource::new(pool.clone()))
        .with_resource(tools::OrderResource::new(pool.clone()))
        .build()?;

    server_common::create_http_server(server)
        .serve("0.0.0.0:3000")
        .await
}
```

## Testing Your Database Server

```bash
# Start the server
DATABASE_URL=sqlite:./chinook.db cargo run --package sales-data

# Test with MCP Inspector
npx @anthropic-ai/mcp-inspector http://localhost:3000
```

Try these queries:
- "List all tables in the database"
- "Show me the top 10 customers by revenue"
- "What were sales by region last quarter?"

## Security Checklist

Before deploying a database MCP server:

- [ ] Only SELECT queries allowed
- [ ] Parameterized queries for all user input
- [ ] Row limits enforced
- [ ] Sensitive columns filtered (SSN, passwords)
- [ ] Connection pooling configured
- [ ] Query timeout set
- [ ] Audit logging enabled
- [ ] OAuth authentication required

## Exercises

1. **Add a sample_data tool**: Return N random rows from a table for exploration

2. **Add query explain**: Wrap queries with EXPLAIN to show execution plan

3. **Add query history**: Track and recall previous queries

4. **Build a PostgreSQL version**: Adapt the server for PostgreSQL

---

## Knowledge Check

Test your understanding of database MCP servers:

{{#quiz ../quizzes/ch03-database.toml}}

---

*Continue to [The Enterprise Data Access Problem](./ch03-01-data-access.md) →*
