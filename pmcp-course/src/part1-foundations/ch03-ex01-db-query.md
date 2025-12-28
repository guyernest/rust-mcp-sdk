# Exercise: Building a Database Query Tool

::: exercise
id: ch03-01-db-query-basics
difficulty: intermediate
time: 35 minutes
prerequisites: [ch02-01-hello-mcp, ch02-02-calculator]
:::

Database access is the "killer app" for enterprise MCP servers. Build a database query tool that lists tables and executes read-only SQL queries.

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
:::

## Your Task

Build two tools:
1. **list_tables** - Returns all tables with their row counts
2. **execute_query** - Runs read-only SQL queries with result limiting

::: starter file="src/main.rs" language=rust
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::Result;
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
    truncated: bool,
}

async fn list_tables(pool: &DbPool) -> Result<Vec<TableInfo>> {
    // TODO: Query sqlite_master for table names
    // Then get row count for each table
    todo!()
}

async fn execute_query(pool: &DbPool, input: QueryInput) -> Result<QueryResult> {
    // TODO: Implement query execution
    // 1. Validate the query starts with SELECT
    // 2. Add LIMIT clause if not present
    // 3. Execute and return structured results
    todo!()
}

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:course.db".to_string());

    let pool: DbPool = Arc::new(
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?
    );

    // TODO: Build server with list_tables and query tools
    todo!()
}
```
:::

::: hint level=1 title="Query sqlite_master"
```rust
let tables: Vec<(String,)> = sqlx::query_as(
    "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
)
.fetch_all(pool.as_ref())
.await?;
```
:::

::: hint level=2 title="Validate SELECT Only"
```rust
let trimmed = input.query.trim().to_uppercase();
if !trimmed.starts_with("SELECT") {
    return Err(anyhow::anyhow!("Only SELECT queries are allowed"));
}
```
:::

::: hint level=3 title="Complete list_tables"
```rust
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
```
:::

::: solution
```rust
use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::Result;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions, Row, Column};
use std::sync::Arc;

type DbPool = Arc<Pool<Sqlite>>;

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

async fn execute_query(pool: &DbPool, input: QueryInput) -> Result<QueryResult> {
    let trimmed = input.query.trim().to_uppercase();
    if !trimmed.starts_with("SELECT") {
        return Err(anyhow::anyhow!("Only SELECT queries are allowed"));
    }

    let query = if !trimmed.contains("LIMIT") {
        format!("{} LIMIT {}", input.query, input.limit + 1)
    } else {
        input.query.clone()
    };

    let rows = sqlx::query(&query)
        .fetch_all(pool.as_ref())
        .await?;

    let truncated = rows.len() > input.limit as usize;
    let rows: Vec<_> = rows.into_iter().take(input.limit as usize).collect();

    let columns: Vec<String> = if !rows.is_empty() {
        rows[0].columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        vec![]
    };

    let json_rows: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            (0..columns.len()).map(|i| {
                if let Ok(v) = row.try_get::<i64, _>(i) {
                    serde_json::Value::Number(v.into())
                } else if let Ok(v) = row.try_get::<String, _>(i) {
                    serde_json::Value::String(v)
                } else {
                    serde_json::Value::Null
                }
            }).collect()
        })
        .collect();

    Ok(QueryResult {
        columns,
        rows: json_rows,
        row_count: json_rows.len(),
        truncated,
    })
}
```

### Key Patterns

- **Arc<Pool<Sqlite>>** - Shared connection pool across tools
- **SELECT validation** - First line of defense
- **Fetch limit+1** - Detect truncation without COUNT query
- **Flexible type handling** - SQLite is dynamically typed
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_select_only() {
        let pool = create_test_pool().await;
        let input = QueryInput {
            query: "DROP TABLE users".to_string(),
            limit: 10,
        };
        assert!(execute_query(&pool, input).await.is_err());
    }

    #[tokio::test]
    async fn test_truncation() {
        let pool = create_test_pool().await;
        let input = QueryInput {
            query: "SELECT * FROM users".to_string(),
            limit: 5,
        };
        let result = execute_query(&pool, input).await.unwrap();
        // If there are more than 5 users, truncated should be true
    }
}
```
:::

::: reflection
- Why do we use Arc to share the database pool between tools?
- What are the limitations of checking 'starts_with SELECT'?
- How does the truncation feedback help an AI assistant?
:::
