# Exercise: Pagination Patterns

::: exercise
id: ch03-03-pagination-patterns
difficulty: intermediate
time: 30 minutes
prerequisites: [ch03-01-db-query-basics]
:::

Learn to handle large result sets safely with cursor-based pagination.

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
:::

## The Problem

**Offset pagination gets slower as you go deeper:**
```sql
-- Fast
SELECT * FROM users LIMIT 100 OFFSET 0

-- Slow - must skip 999,900 rows
SELECT * FROM users LIMIT 100 OFFSET 999900
```

**Cursor pagination is O(1):**
```sql
-- Same speed regardless of position
SELECT * FROM users WHERE id > 999900 ORDER BY id LIMIT 100
```

::: starter file="src/main.rs" language=rust
```rust
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[derive(Serialize, Deserialize)]
struct Cursor {
    last_id: i64,
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

#[derive(Deserialize, JsonSchema)]
struct PaginatedQueryInput {
    table: String,
    page_size: Option<i32>,
    cursor: Option<String>,
}

#[derive(Serialize)]
struct PaginatedResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    count: usize,
    next_cursor: Option<String>,
    status: String,
}

const ALLOWED_TABLES: &[&str] = &["users", "orders", "products"];

async fn paginated_query(pool: &DbPool, input: PaginatedQueryInput) -> Result<PaginatedResult> {
    // TODO: Implement cursor-based pagination
    // 1. Validate table is in allowlist
    // 2. Decode cursor if provided
    // 3. Query with WHERE id > last_id ORDER BY id LIMIT n+1
    // 4. Create next_cursor from last row if more data exists
    // 5. Return human-readable status for AI
    todo!()
}
```
:::

::: hint level=1 title="Fetch N+1 Pattern"
Fetch `page_size + 1` rows. If you get more than `page_size`, there are more pages.
```rust
let has_more = rows.len() > page_size;
let rows = rows.into_iter().take(page_size).collect();
```
:::

::: hint level=2 title="Include Table in Cursor"
Store the table name in the cursor for security - prevents using a cursor from table A to query table B.
```rust
if cursor.table != input.table {
    return Err(anyhow!("Cursor table mismatch"));
}
```
:::

::: solution
```rust
async fn paginated_query(pool: &DbPool, input: PaginatedQueryInput) -> Result<PaginatedResult> {
    // 1. Validate table
    if !ALLOWED_TABLES.contains(&input.table.as_str()) {
        return Err(anyhow!("Table '{}' not allowed", input.table));
    }

    let page_size = input.page_size.unwrap_or(50).min(100);

    // 2. Decode cursor
    let start_id = match &input.cursor {
        Some(c) => {
            let cursor = Cursor::decode(c)?;
            if cursor.table != input.table {
                return Err(anyhow!("Cursor was for different table"));
            }
            cursor.last_id
        }
        None => 0,
    };

    // 3. Query with cursor
    let query = format!(
        "SELECT * FROM {} WHERE id > ? ORDER BY id LIMIT ?",
        input.table
    );

    let rows = sqlx::query(&query)
        .bind(start_id)
        .bind(page_size + 1)
        .fetch_all(pool.as_ref())
        .await?;

    // 4. Check for more
    let has_more = rows.len() > page_size as usize;
    let rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();

    // 5. Create next cursor
    let next_cursor = if has_more && !rows.is_empty() {
        let last_id = rows.last().unwrap().try_get::<i64, _>("id")?;
        Some(Cursor { last_id, table: input.table.clone() }.encode())
    } else {
        None
    };

    let count = rows.len();

    // Get columns
    let columns: Vec<String> = if !rows.is_empty() {
        rows[0].columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        vec![]
    };

    // Convert to JSON
    let json_rows: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            (0..columns.len()).map(|i| {
                row.try_get::<i64, _>(i)
                    .map(|v| serde_json::Value::Number(v.into()))
                    .or_else(|_| row.try_get::<String, _>(i).map(serde_json::Value::String))
                    .unwrap_or(serde_json::Value::Null)
            }).collect()
        })
        .collect();

    // 6. Human-readable status
    let status = if count == 0 {
        "No results found.".to_string()
    } else if next_cursor.is_some() {
        format!("Returned {} rows. More data available - use next_cursor to continue.", count)
    } else {
        format!("Returned {} rows. This is all data.", count)
    };

    Ok(PaginatedResult {
        columns,
        rows: json_rows,
        count,
        next_cursor,
        status,
    })
}
```

### Key Patterns

1. **Opaque cursors** - base64 JSON hides implementation details
2. **Fetch N+1** - efficiently detect if more pages exist
3. **Table in cursor** - security boundary validation
4. **Human-readable status** - helps AI understand pagination state
:::

::: reflection
- Why do we include the table name in the cursor?
- What would happen if rows were deleted between page fetches?
- How would you support sorting by a non-unique column?
:::
