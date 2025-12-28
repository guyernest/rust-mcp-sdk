# Handling Large Results

Enterprise databases contain millions of rows. When Claude asks "Show me all customers," you can't return everything at once. This section covers patterns for handling large result sets safely and efficiently.

## The Problem with Large Results

Returning too much data causes multiple problems:

| Problem | Impact |
|---------|--------|
| **Memory exhaustion** | Server crashes with OOM |
| **Slow responses** | Users wait forever |
| **Context overflow** | AI can't process millions of rows |
| **Network costs** | Unnecessary data transfer |
| **Poor UX** | Information overload |

## Pagination Strategies

### Strategy 1: Offset Pagination (Simple but Limited)

```sql
SELECT * FROM customers ORDER BY id LIMIT 100 OFFSET 0    -- Page 1
SELECT * FROM customers ORDER BY id LIMIT 100 OFFSET 100  -- Page 2
SELECT * FROM customers ORDER BY id LIMIT 100 OFFSET 200  -- Page 3
```

**Implementation:**

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct OffsetPaginatedInput {
    pub query: String,
    
    #[serde(default = "default_page")]
    pub page: i32,
    
    #[serde(default = "default_page_size")]
    pub page_size: i32,
}

fn default_page() -> i32 { 0 }
fn default_page_size() -> i32 { 50 }

#[derive(Debug, Serialize, JsonSchema)]
pub struct OffsetPaginatedOutput {
    pub rows: Vec<Vec<serde_json::Value>>,
    pub columns: Vec<String>,
    pub page: i32,
    pub page_size: i32,
    pub has_more: bool,
}

async fn paginated_query(pool: &DbPool, input: OffsetPaginatedInput) -> Result<OffsetPaginatedOutput> {
    let page_size = input.page_size.min(100);  // Cap at 100
    let offset = input.page * page_size;
    
    // Fetch one extra to detect if there are more
    let query = format!(
        "{} LIMIT {} OFFSET {}",
        input.query.trim_end_matches(';'),
        page_size + 1,
        offset
    );
    
    let rows = execute_query(pool, &query).await?;
    let has_more = rows.len() > page_size as usize;
    let rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
    
    Ok(OffsetPaginatedOutput {
        rows,
        columns: vec![],  // Extract from first row
        page: input.page,
        page_size,
        has_more,
    })
}
```

**Problems with offset pagination:**

```
Page 1:     OFFSET 0    → Scans 100 rows      ✓ Fast
Page 100:   OFFSET 9900 → Scans 10,000 rows   ⚠ Slow
Page 10000: OFFSET 999900 → Scans 1M rows    ✗ Very slow
```

The database must skip all offset rows before returning results. This gets slower as you paginate deeper.

### Strategy 2: Cursor Pagination (Recommended)

Cursor pagination uses the last seen value to fetch the next page:

```sql
-- First page
SELECT * FROM customers ORDER BY id LIMIT 100

-- Next page (where 12345 was the last ID)
SELECT * FROM customers WHERE id > 12345 ORDER BY id LIMIT 100
```

This is **O(1)** regardless of how deep you paginate—the database uses an index seek, not a scan.

**Implementation:**

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Opaque cursor containing pagination state
#[derive(Debug, Serialize, Deserialize)]
struct Cursor {
    /// The last seen ID
    last_id: i64,
    /// Table name (for validation)
    table: String,
    /// Sort column
    sort_column: String,
    /// Sort direction
    ascending: bool,
}

impl Cursor {
    /// Encode cursor to opaque string
    fn encode(&self) -> String {
        let json = serde_json::to_string(self).unwrap();
        BASE64.encode(json.as_bytes())
    }
    
    /// Decode cursor from opaque string
    fn decode(encoded: &str) -> Result<Self> {
        let bytes = BASE64.decode(encoded)
            .map_err(|_| anyhow!("Invalid cursor"))?;
        let json = String::from_utf8(bytes)
            .map_err(|_| anyhow!("Invalid cursor encoding"))?;
        serde_json::from_str(&json)
            .map_err(|_| anyhow!("Invalid cursor format"))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CursorPaginatedInput {
    /// Table to query
    pub table: String,
    
    /// Number of results per page (max 100)
    #[serde(default = "default_page_size")]
    pub page_size: i32,
    
    /// Cursor from previous response (omit for first page)
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CursorPaginatedOutput {
    pub rows: Vec<serde_json::Value>,
    pub columns: Vec<String>,
    pub count: usize,
    
    /// Cursor to fetch next page (null if no more data)
    pub next_cursor: Option<String>,
    
    /// Human-readable pagination status
    pub status: String,
}

const ALLOWED_TABLES: &[&str] = &["customers", "orders", "products", "invoices"];

async fn cursor_paginated_query(
    pool: &DbPool,
    input: CursorPaginatedInput,
) -> Result<CursorPaginatedOutput> {
    // Validate table
    if !ALLOWED_TABLES.contains(&input.table.as_str()) {
        return Err(anyhow!("Table '{}' not allowed", input.table));
    }
    
    let page_size = input.page_size.min(100);
    
    // Decode cursor if provided
    let (start_id, sort_col, ascending) = match &input.cursor {
        Some(cursor_str) => {
            let cursor = Cursor::decode(cursor_str)?;
            
            // Validate cursor is for the same table
            if cursor.table != input.table {
                return Err(anyhow!("Cursor is for different table"));
            }
            
            (cursor.last_id, cursor.sort_column, cursor.ascending)
        }
        None => (0, "id".to_string(), true),
    };
    
    // Build query with cursor condition
    let comparison = if ascending { ">" } else { "<" };
    let order = if ascending { "ASC" } else { "DESC" };
    
    let query = format!(
        "SELECT * FROM {} WHERE {} {} ? ORDER BY {} {} LIMIT ?",
        input.table,
        sort_col,
        comparison,
        start_id,
        sort_col,
        order
    );
    
    let rows = sqlx::query(&query)
        .bind(start_id)
        .bind(page_size + 1)  // Fetch one extra to detect more
        .fetch_all(pool.as_ref())
        .await?;
    
    let has_more = rows.len() > page_size as usize;
    let rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
    
    // Create next cursor if there are more rows
    let next_cursor = if has_more && !rows.is_empty() {
        let last_row = rows.last().unwrap();
        let last_id: i64 = last_row.try_get(&sort_col)?;
        
        Some(Cursor {
            last_id,
            table: input.table.clone(),
            sort_column: sort_col,
            ascending,
        }.encode())
    } else {
        None
    };
    
    let count = rows.len();
    let status = if count == 0 {
        "No results found.".to_string()
    } else if next_cursor.is_some() {
        format!("Showing {} results. Use next_cursor to see more.", count)
    } else {
        format!("Showing all {} results.", count)
    };
    
    Ok(CursorPaginatedOutput {
        rows: convert_rows(rows),
        columns: vec![],  // Extract from schema
        count,
        next_cursor,
        status,
    })
}
```

### Why Include Table in Cursor?

The cursor includes the table name for security:

```rust
// Attacker tries to use a customers cursor on the users table
cursor = { last_id: 12345, table: "customers", ... }
input.table = "users"  // Trying to access different table

// Validation catches this:
if cursor.table != input.table {
    return Err(anyhow!("Cursor is for different table"));
}
```

Without this check, an attacker could:
1. Get a cursor for a public table
2. Use it to paginate through a private table

## Streaming Large Results

For very large exports, consider streaming:

```rust
use futures::StreamExt;

async fn stream_query(
    pool: &DbPool,
    query: &str,
) -> impl futures::Stream<Item = Result<serde_json::Value>> {
    sqlx::query(query)
        .fetch(pool.as_ref())
        .map(|row_result| {
            row_result
                .map(|row| row_to_json(&row))
                .map_err(|e| anyhow!("Row error: {}", e))
        })
}

// Usage for large exports
async fn export_table(pool: &DbPool, table: &str, output: &mut File) -> Result<()> {
    let query = format!("SELECT * FROM {}", table);
    let mut stream = stream_query(pool, &query);
    
    while let Some(row_result) = stream.next().await {
        let row = row_result?;
        writeln!(output, "{}", serde_json::to_string(&row)?)?;
    }
    
    Ok(())
}
```

**Note:** Streaming isn't directly supported in MCP responses, but you can use it for:
- File exports
- Background processing
- Chunked responses (if your transport supports it)

## Memory-Safe Patterns

### Pattern 1: Always Limit

```rust
const MAX_ROWS: i32 = 10_000;
const DEFAULT_ROWS: i32 = 100;

fn safe_limit(requested: Option<i32>) -> i32 {
    requested
        .unwrap_or(DEFAULT_ROWS)
        .min(MAX_ROWS)
        .max(1)  // At least 1
}
```

### Pattern 2: Early Termination

```rust
async fn fetch_limited(pool: &DbPool, query: &str, max: usize) -> Result<Vec<Row>> {
    let mut rows = Vec::with_capacity(max);
    let mut stream = sqlx::query(query).fetch(pool.as_ref());
    
    while let Some(row) = stream.next().await {
        rows.push(row?);
        if rows.len() >= max {
            break;  // Stop fetching, even if more exist
        }
    }
    
    Ok(rows)
}
```

### Pattern 3: Result Size Estimation

```rust
async fn check_result_size(pool: &DbPool, query: &str) -> Result<i64> {
    // Wrap query in COUNT to check size first
    let count_query = format!(
        "SELECT COUNT(*) FROM ({}) as subquery",
        query.trim_end_matches(';')
    );
    
    let count: (i64,) = sqlx::query_as(&count_query)
        .fetch_one(pool.as_ref())
        .await?;
    
    Ok(count.0)
}

async fn safe_query(pool: &DbPool, query: &str, limit: i32) -> Result<QueryOutput> {
    let estimated_size = check_result_size(pool, query).await?;
    
    if estimated_size > 100_000 {
        return Err(anyhow!(
            "Query would return {} rows. Please add filters or use pagination.",
            estimated_size
        ));
    }
    
    // Proceed with actual query
    execute_query(pool, query, limit).await
}
```

## AI-Friendly Pagination Messages

Help Claude understand pagination state:

```rust
fn pagination_message(count: usize, total: Option<i64>, has_more: bool) -> String {
    match (total, has_more) {
        (Some(t), true) => format!(
            "Showing {} of {} total results. Use the next_cursor to fetch more.",
            count, t
        ),
        (Some(t), false) => format!(
            "Showing all {} results.",
            t
        ),
        (None, true) => format!(
            "Showing {} results. More are available - use next_cursor to continue.",
            count
        ),
        (None, false) => format!(
            "Showing {} results. This is the complete result set.",
            count
        ),
    }
}
```

Claude can then naturally say:
> "I found 50 customers matching your criteria. There are more results available. Would you like me to fetch the next page?"

## Performance Comparison

| Approach | Page 1 | Page 100 | Page 10,000 |
|----------|--------|----------|-------------|
| No pagination | ✗ OOM | ✗ OOM | ✗ OOM |
| OFFSET | 10ms | 100ms | 5000ms |
| Cursor | 10ms | 10ms | 10ms |

Cursor pagination maintains constant performance regardless of depth.

## When to Use Each Strategy

| Scenario | Recommended Strategy |
|----------|---------------------|
| Simple UI pagination | Offset (if depth < 100 pages) |
| API pagination | Cursor |
| Search results | Cursor |
| Infinite scroll | Cursor |
| Admin data export | Streaming |
| Real-time feeds | Cursor + polling |

## Complete Pagination Implementation

```rust
/// Paginated query tool with cursor-based pagination
pub struct PaginatedQuery {
    pool: DbPool,
}

impl PaginatedQuery {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn into_tool(self) -> TypedTool<CursorPaginatedInput, CursorPaginatedOutput> {
        let pool = self.pool.clone();
        
        TypedTool::new(
            "paginated_query",
            "Query a table with cursor-based pagination. Returns a cursor for fetching additional pages.",
            move |input: CursorPaginatedInput| {
                let pool = pool.clone();
                Box::pin(async move {
                    cursor_paginated_query(&pool, input).await
                })
            },
        )
    }
}
```

## Summary

| Problem | Solution |
|---------|----------|
| Too many rows | Always use LIMIT |
| Deep pagination slow | Use cursor pagination |
| Memory exhaustion | Stream or chunk |
| AI can't process all data | Provide clear pagination status |
| Cursor tampering | Include table in cursor, validate |

---

*Continue to [Chapter 3 Exercises](./ch03-exercises.md) to practice these patterns →*
