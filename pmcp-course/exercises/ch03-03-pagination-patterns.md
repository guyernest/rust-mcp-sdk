# Implementing Pagination for Large Results

**Type:** Implementation  
**Difficulty:** Intermediate  
**Estimated Time:** 30 minutes  
**Prerequisites:** ch03-01-db-query-basics

---

## Overview

Your database query tool from the previous exercise works great for small result sets, but what happens when a table has millions of rows? Without proper pagination:

- **Memory exhaustion:** Loading 10M rows into memory crashes your server
- **Timeouts:** Long queries block the connection pool
- **Poor UX:** AI assistants can't process massive JSON responses effectively

This exercise teaches cursor-based pagination - the production pattern for handling large datasets efficiently. You'll learn why it's superior to offset-based pagination and how to implement it safely.

---

## Learning Objectives

### Thinking
- Why offset pagination fails at scale (OFFSET 1000000 is slow)
- How cursor-based pagination maintains consistent performance
- Tradeoffs between different pagination strategies

### Doing
- Implement cursor-based pagination with a 'next' token
- Handle edge cases (empty results, last page, invalid cursors)
- Design API responses that guide AI assistants to fetch more

---

## Discussion Questions

Before starting, consider:

1. If you have 10 million rows and an AI asks for "all customers", what should happen?
2. Why is `OFFSET 999000 LIMIT 1000` slower than `WHERE id > 999000 LIMIT 1000`?
3. How should an MCP response indicate that more data is available?
4. What makes a good pagination cursor? (hint: not just a page number)

---

## Concepts

### Offset-Based Pagination (The Problem)

```sql
-- Page 1
SELECT * FROM users LIMIT 100 OFFSET 0

-- Page 10000
SELECT * FROM users LIMIT 100 OFFSET 999900
```

**Problem:** The database must scan and skip 999,900 rows for page 10000. Performance degrades linearly with offset.

### Cursor-Based Pagination (The Solution)

```sql
-- Page 1
SELECT * FROM users WHERE id > 0 ORDER BY id LIMIT 100

-- Next page (cursor contains last_id = 100)
SELECT * FROM users WHERE id > 100 ORDER BY id LIMIT 100
```

**Solution:** Uses an index to jump directly to the starting point. O(1) performance regardless of position.

---

## Starter Code

```rust
//! Paginated Database Query Tool
//!
//! Demonstrates production-ready pagination patterns for large datasets.

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

---

## Hints

### Level 1: Validate the table

```rust
if !ALLOWED_TABLES.contains(&input.table.as_str()) {
    return Err(anyhow::anyhow!("Table not allowed"));
}
```

### Level 2: Build query with cursor support

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

### Level 3: Complete implementation with has_more detection

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

    // Build and execute query
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

    // Build result...
}
```

---

## Tests

Your implementation should pass these tests:

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

---

## Solution Explanation

The complete solution demonstrates these key patterns:

### 1. Opaque Cursors
We encode cursor state as base64 JSON. This:
- Hides implementation details from clients
- Allows adding fields without breaking clients
- Validates that cursor matches the current query

### 2. Fetch N+1 Pattern
```rust
let rows = ... LIMIT page_size + 1
let has_more = rows.len() > page_size;
let rows = rows.into_iter().take(page_size);
```
This efficiently detects whether more pages exist without an extra COUNT query.

### 3. Table Validation in Cursor
The cursor includes the table name to prevent attacks like:
- Get cursor from `users` table
- Use it to paginate `admin_secrets` table

### 4. Human-Readable Status
The `status` field helps AI assistants understand pagination:
- "More data available - use next_cursor"
- "This is all available data"

---

## Reflection Questions

1. Why do we include the table name in the cursor?
2. What would happen if rows were deleted between page fetches?
3. How would you support sorting by a non-unique column?
4. Why is the cursor base64-encoded JSON instead of just an ID?

---

## Production Considerations

Real-world pagination often needs:
- Support for non-integer primary keys (UUIDs)
- Compound cursors for multi-column sorting
- Cursor expiration for long-lived operations
- Total count estimates (not exact, for UI)
- Support for backward pagination (previous page)

---

## Next Steps

After completing this exercise:
- Consider how pagination affects the AI conversation experience
- Think about caching strategies for repeated queries
- Explore how different databases handle large result sets
