::: exercise
id: ch03-03-pagination-patterns
type: implementation
difficulty: intermediate
time: 30 minutes
prerequisites: ch03-01-db-query-basics
:::

Your database query tool from the previous exercise works great for small result sets, but what happens when a table has millions of rows? Without proper pagination:

- **Memory exhaustion:** Loading 10M rows into memory crashes your server
- **Timeouts:** Long queries block the connection pool
- **Poor UX:** AI assistants can't process massive JSON responses effectively

This exercise teaches cursor-based pagination - the production pattern for handling large datasets efficiently. You'll learn why it's superior to offset-based pagination and how to implement it safely.

::: objectives
thinking:
  - Why offset pagination fails at scale (OFFSET 1000000 is slow)
  - How cursor-based pagination maintains consistent performance
  - Tradeoffs between different pagination strategies
doing:
  - Implement cursor-based pagination with a 'next' token
  - Handle edge cases (empty results, last page, invalid cursors)
  - Design API responses that guide AI assistants to fetch more
:::

::: discussion
- If you have 10 million rows and an AI asks for "all customers", what should happen?
- Why is `OFFSET 999000 LIMIT 1000` slower than `WHERE id > 999000 LIMIT 1000`?
- How should an MCP response indicate that more data is available?
- What makes a good pagination cursor? (hint: not just a page number)
:::

::: starter file="src/main.rs" language=rust
```rust
//! Paginated Database Query Tool
//!
//! Demonstrates production-ready pagination patterns for large datasets.
//!
//! CONCEPT: Offset vs Cursor Pagination
//!
//! Offset-Based (SLOW at scale):
//!   SELECT * FROM users LIMIT 100 OFFSET 999900
//!   // Database must scan and skip 999,900 rows!
//!
//! Cursor-Based (O(1) performance):
//!   SELECT * FROM users WHERE id > 999900 ORDER BY id LIMIT 100
//!   // Uses index to jump directly to starting point

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use anyhow::Result;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions, Row, Column};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

type DbPool = Arc<Pool<Sqlite>>;

/// Input for paginated query
#[derive(Deserialize, JsonSchema)]
struct PaginatedQueryInput {
    /// Table to query (from allowlist)
    table: String,

    /// Columns to select (optional, defaults to all)
    columns: Option<Vec<String>>,

    /// Number of rows per page (max 100)
    #[serde(default = "default_page_size")]
    page_size: i32,

    /// Cursor from previous response (omit for first page)
    cursor: Option<String>,
}

fn default_page_size() -> i32 { 50 }

/// Output with pagination information
#[derive(Serialize)]
struct PaginatedResult {
    /// Column names
    columns: Vec<String>,

    /// Row data
    rows: Vec<Vec<serde_json::Value>>,

    /// Number of rows in this page
    count: usize,

    /// Cursor for next page (null if no more data)
    next_cursor: Option<String>,

    /// Human-readable pagination status for AI
    status: String,
}

/// Internal cursor structure
#[derive(Serialize, Deserialize)]
struct Cursor {
    /// Last ID seen
    last_id: i64,
    /// Table being queried (for validation)
    table: String,
}

impl Cursor {
    fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap();
        BASE64.encode(json.as_bytes())
    }

    fn decode(encoded: &str) -> Result<Self> {
        let bytes = BASE64.decode(encoded)?;
        let json = String::from_utf8(bytes)?;
        Ok(serde_json::from_str(&json)?)
    }
}

// Allowlisted tables for security
const ALLOWED_TABLES: &[&str] = &["users", "orders", "products", "customers"];

/// Execute a paginated query
async fn paginated_query(pool: &DbPool, input: PaginatedQueryInput) -> Result<PaginatedResult> {
    // TODO: Implement paginated query
    //
    // Steps:
    // 1. Validate table is in allowlist
    // 2. If cursor provided, decode and validate it
    // 3. Build query with WHERE id > last_id ORDER BY id LIMIT page_size+1
    // 4. Execute query and extract results
    // 5. Check if there are more results (fetched page_size + 1)
    // 6. If more results, create next_cursor with last row's ID
    // 7. Return PaginatedResult with appropriate status message

    // Hints:
    // - Use the Cursor struct to encode/decode cursor state
    // - Fetch page_size + 1 rows to detect if more pages exist
    // - The status field should help AI understand pagination state

    todo!("Implement paginated_query")
}

/// Get total count for a table (for context)
async fn get_table_count(pool: &DbPool, table: &str) -> Result<i64> {
    if !ALLOWED_TABLES.contains(&table) {
        return Err(anyhow::anyhow!("Table not allowed: {}", table));
    }

    let count: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool.as_ref())
        .await?;

    Ok(count.0)
}

#[tokio::main]
async fn main() -> Result<()> {
    let pool: DbPool = Arc::new(
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite:sample.db")
            .await?
    );

    let pool_query = pool.clone();

    let server = Server::builder()
        .name("paginated-db")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("query", TypedTool::new("query", move |input: PaginatedQueryInput| {
            let pool = pool_query.clone();
            Box::pin(async move {
                let result = paginated_query(&pool, input).await?;
                Ok(serde_json::to_value(result)?)
            })
        }))
        .build()?;

    println!("Paginated database server ready!");
    Ok(())
}
```
:::

::: hint level=1
Start by validating the table is in the allowlist:

```rust
if !ALLOWED_TABLES.contains(&input.table.as_str()) {
    return Err(anyhow::anyhow!("Table not allowed"));
}
```
:::

::: hint level=2
Build the query with cursor support:

```rust
let start_id = if let Some(cursor_str) = &input.cursor {
    let cursor = Cursor::decode(cursor_str)?;
    if cursor.table != input.table {
        return Err(anyhow::anyhow!("Cursor table mismatch"));
    }
    cursor.last_id
} else {
    0
};

let query = format!(
    "SELECT * FROM {} WHERE id > {} ORDER BY id LIMIT {}",
    input.table, start_id, input.page_size + 1
);
```
:::

::: hint level=3
Complete implementation with has_more detection:

```rust
async fn paginated_query(pool: &DbPool, input: PaginatedQueryInput) -> Result<PaginatedResult> {
    // Validate table
    if !ALLOWED_TABLES.contains(&input.table.as_str()) {
        return Err(anyhow::anyhow!("Table '{}' not allowed", input.table));
    }

    // Limit page size
    let page_size = input.page_size.min(100);

    // Decode cursor
    let start_id = match &input.cursor {
        Some(c) => {
            let cursor = Cursor::decode(c)?;
            if cursor.table != input.table {
                return Err(anyhow::anyhow!("Cursor was for different table"));
            }
            cursor.last_id
        }
        None => 0,
    };

    // Build and execute query - fetch N+1 to detect more pages
    let query = format!(
        "SELECT * FROM {} WHERE id > {} ORDER BY id LIMIT {}",
        input.table, start_id, page_size + 1
    );

    let rows = sqlx::query(&query)
        .fetch_all(pool.as_ref())
        .await?;

    // Check for more results
    let has_more = rows.len() > page_size as usize;
    let rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();

    // Build next_cursor if more pages exist...
}
```
:::

::: solution reveal=on-demand
```rust
async fn paginated_query(pool: &DbPool, input: PaginatedQueryInput) -> Result<PaginatedResult> {
    // Validate table is in allowlist
    if !ALLOWED_TABLES.contains(&input.table.as_str()) {
        return Err(anyhow::anyhow!("Table '{}' not in allowlist", input.table));
    }

    // Limit page size to max 100
    let page_size = input.page_size.min(100).max(1);

    // Decode cursor if provided
    let start_id = match &input.cursor {
        Some(cursor_str) => {
            let cursor = Cursor::decode(cursor_str)?;
            // Validate cursor is for same table (security check)
            if cursor.table != input.table {
                return Err(anyhow::anyhow!(
                    "Cursor was created for table '{}', not '{}'",
                    cursor.table, input.table
                ));
            }
            cursor.last_id
        }
        None => 0,
    };

    // Build query - fetch page_size + 1 to detect if more pages exist
    let query = format!(
        "SELECT * FROM {} WHERE id > ? ORDER BY id LIMIT ?",
        input.table
    );

    let all_rows = sqlx::query(&query)
        .bind(start_id)
        .bind(page_size + 1)
        .fetch_all(pool.as_ref())
        .await?;

    // Determine if there are more results
    let has_more = all_rows.len() > page_size as usize;
    let rows: Vec<_> = all_rows.into_iter().take(page_size as usize).collect();

    // Extract column names
    let columns: Vec<String> = if let Some(first_row) = rows.first() {
        first_row.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        vec![]
    };

    // Convert rows to JSON values
    let row_data: Vec<Vec<serde_json::Value>> = rows.iter().map(|row| {
        columns.iter().enumerate().map(|(i, _)| {
            // Try to get as different types
            if let Ok(v) = row.try_get::<i64, _>(i) {
                serde_json::Value::Number(v.into())
            } else if let Ok(v) = row.try_get::<String, _>(i) {
                serde_json::Value::String(v)
            } else {
                serde_json::Value::Null
            }
        }).collect()
    }).collect();

    // Get last ID for cursor
    let last_id = row_data.last()
        .and_then(|row| row.first())
        .and_then(|v| v.as_i64());

    // Create next cursor if more data exists
    let next_cursor = if has_more {
        last_id.map(|id| Cursor {
            last_id: id,
            table: input.table.clone(),
        }.encode())
    } else {
        None
    };

    // Human-readable status for AI
    let status = if has_more {
        format!(
            "Showing {} rows. More data available - pass next_cursor to continue.",
            row_data.len()
        )
    } else {
        format!("Showing {} rows. This is all available data.", row_data.len())
    };

    Ok(PaginatedResult {
        columns,
        rows: row_data,
        count: row_data.len(),
        next_cursor,
        status,
    })
}

// Key patterns demonstrated:
// 1. Opaque Cursors - base64 JSON hides implementation details
// 2. Fetch N+1 Pattern - efficiently detect more pages without COUNT
// 3. Table Validation in Cursor - prevent cursor reuse attacks
// 4. Human-Readable Status - helps AI understand pagination state
```
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_first_page() {
        // First page should return results and a next_cursor
    }

    #[tokio::test]
    async fn test_continue_with_cursor() {
        // Second page should have no overlap with first
    }

    #[tokio::test]
    async fn test_last_page() {
        // Final page should have no next_cursor
    }

    #[tokio::test]
    async fn test_invalid_table() {
        // Tables not in allowlist should error
    }

    #[tokio::test]
    async fn test_cursor_table_mismatch() {
        // Cursor from table A shouldn't work for table B
    }
}
```
:::

::: reflection
- Why do we include the table name in the cursor?
- What would happen if rows were deleted between page fetches?
- How would you support sorting by a non-unique column?
- Why is the cursor base64-encoded JSON instead of just an ID?
:::
