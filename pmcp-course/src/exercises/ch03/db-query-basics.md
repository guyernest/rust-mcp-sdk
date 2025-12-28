::: exercise
id: ch03-01-db-query-basics
type: implementation
difficulty: intermediate
time: 35 minutes
prerequisites: ch02-01-hello-mcp, ch02-02-calculator
:::

Database access is the "killer app" for enterprise MCP servers. When employees
need data for AI conversations, they shouldn't have to export CSVs and paste
into chat windows. An MCP server can provide secure, direct access.

In this exercise, you'll build a database query tool that:
1. Lists available tables
2. Executes read-only SQL queries
3. Returns structured results

::: objectives
thinking:
  - Why read-only access is essential for AI tools
  - How to structure database results for AI consumption
  - The tradeoffs between flexibility and security in query tools
doing:
  - Create tools that interact with SQLite databases
  - Use sqlx for async database operations
  - Structure output for AI-friendly consumption
:::

::: discussion
- Why might you want an AI to query databases directly instead of using pre-built reports?
- What's the risk of allowing arbitrary SQL queries? How would you mitigate it?
- How should results be formatted so an AI can understand and explain them?
- What metadata would help an AI write better queries?
:::

::: starter file="src/main.rs"
```rust
//! Database Query MCP Server
//!
//! Provides read-only access to a SQLite database through MCP tools.

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::Result;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions, Row};
use std::sync::Arc;

type DbPool = Arc<Pool<Sqlite>>;

#[derive(Deserialize, JsonSchema)]
struct ListTablesInput {}

#[derive(Serialize)]
struct TableInfo {
    name: String,
    row_count: i64,
}

#[derive(Deserialize, JsonSchema)]
struct QueryInput {
    /// The SQL query to execute (must be SELECT)
    query: String,
    /// Maximum rows to return (default: 100)
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 { 100 }

#[derive(Serialize)]
struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    row_count: usize,
}

// TODO: Implement list_tables function
async fn list_tables(pool: &DbPool) -> Result<Vec<TableInfo>> {
    // Query sqlite_master for table names
    // For each table, get the row count
    todo!("Implement list_tables")
}

// TODO: Implement execute_query function
async fn execute_query(pool: &DbPool, input: &QueryInput) -> Result<QueryResult> {
    // 1. Validate that query starts with SELECT
    // 2. Add LIMIT clause if not present
    // 3. Execute query and collect results
    // 4. Return structured result
    todo!("Implement execute_query")
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create database connection pool
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data.db".to_string());
    
    let pool = Arc::new(
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?
    );

    let pool_for_tables = pool.clone();
    let pool_for_query = pool.clone();

    let server = Server::builder()
        .name("db-query")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        // TODO: Add list_tables tool
        // TODO: Add execute_query tool
        .build()?;

    println!("Database query server ready!");
    Ok(())
}
```
:::

::: hint level=1 title="Querying SQLite schema"
To list tables in SQLite:
```rust
let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
    .fetch_all(pool.as_ref())
    .await?;
```
:::

::: hint level=2 title="Validating SELECT queries"
Check that the query is read-only:
```rust
let trimmed = input.query.trim().to_uppercase();
if !trimmed.starts_with("SELECT") {
    return Err(anyhow!("Only SELECT queries are allowed"));
}
```
:::

::: hint level=3 title="Complete execute_query"
```rust
async fn execute_query(pool: &DbPool, input: &QueryInput) -> Result<QueryResult> {
    let trimmed = input.query.trim().to_uppercase();
    if !trimmed.starts_with("SELECT") {
        return Err(anyhow!("Only SELECT queries are allowed"));
    }
    
    let query = if !trimmed.contains("LIMIT") {
        format!("{} LIMIT {}", input.query, input.limit)
    } else {
        input.query.clone()
    };
    
    let rows = sqlx::query(&query)
        .fetch_all(pool.as_ref())
        .await?;
    
    // Process rows into structured output
    // ...
}
```
:::

::: solution
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::{Result, anyhow};
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions, Row, Column};
use std::sync::Arc;

type DbPool = Arc<Pool<Sqlite>>;

#[derive(Deserialize, JsonSchema)]
struct ListTablesInput {}

#[derive(Serialize)]
struct TableInfo {
    name: String,
    row_count: i64,
}

#[derive(Deserialize, JsonSchema)]
struct QueryInput {
    query: String,
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 { 100 }

#[derive(Serialize)]
struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    row_count: usize,
}

async fn list_tables(pool: &DbPool) -> Result<Vec<TableInfo>> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(pool.as_ref())
    .await?;
    
    let mut result = Vec::new();
    for (name,) in tables {
        let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", name))
            .fetch_one(pool.as_ref())
            .await?;
        result.push(TableInfo { name, row_count: count.0 });
    }
    
    Ok(result)
}

async fn execute_query(pool: &DbPool, input: &QueryInput) -> Result<QueryResult> {
    let trimmed = input.query.trim().to_uppercase();
    if !trimmed.starts_with("SELECT") {
        return Err(anyhow!("Only SELECT queries are allowed"));
    }
    
    let query = if !trimmed.contains("LIMIT") {
        format!("{} LIMIT {}", input.query, input.limit)
    } else {
        input.query.clone()
    };
    
    let rows = sqlx::query(&query)
        .fetch_all(pool.as_ref())
        .await?;
    
    let columns: Vec<String> = if let Some(row) = rows.first() {
        row.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        vec![]
    };
    
    let data: Vec<Vec<serde_json::Value>> = rows.iter().map(|row| {
        columns.iter().enumerate().map(|(i, _)| {
            row.try_get::<String, _>(i)
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        }).collect()
    }).collect();
    
    Ok(QueryResult {
        row_count: data.len(),
        columns,
        rows: data,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./data.db".to_string());
    
    let pool = Arc::new(
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?
    );

    let pool_for_tables = pool.clone();
    let pool_for_query = pool.clone();

    let server = Server::builder()
        .name("db-query")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("list_tables", TypedTool::new("list_tables", move |_: ListTablesInput| {
            let pool = pool_for_tables.clone();
            Box::pin(async move {
                let tables = list_tables(&pool).await?;
                Ok(serde_json::to_value(tables)?)
            })
        }))
        .tool("execute_query", TypedTool::new("execute_query", move |input: QueryInput| {
            let pool = pool_for_query.clone();
            Box::pin(async move {
                let result = execute_query(&pool, &input).await?;
                Ok(serde_json::to_value(result)?)
            })
        }))
        .build()?;

    println!("Database query server ready!");
    Ok(())
}
```

### Explanation

**Connection Pooling**: Using `Arc<Pool>` allows sharing the connection pool across multiple tool handlers efficiently.

**Read-Only Validation**: Checking for SELECT prevents destructive queries, though this is a basic check - production systems need more robust validation.

**Result Structuring**: Returning columns and rows separately helps AI understand the data schema.

**LIMIT Enforcement**: Adding a default LIMIT prevents accidentally returning millions of rows.
:::

::: reflection
- What SQL injection risks remain even with SELECT-only validation?
- How would you handle different data types (integers, dates, blobs)?
- What additional metadata would help an AI write better queries?
- How would you add pagination for large result sets?
:::
