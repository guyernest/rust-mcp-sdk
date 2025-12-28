# SQL Safety and Injection Prevention

SQL injection is consistently in the OWASP Top 10 vulnerabilities. When you build a database MCP server, you're creating an interface between AI-generated queries and your production data. Security isn't optional—it's essential.

## Understanding SQL Injection

SQL injection occurs when untrusted input is concatenated into SQL queries:

```rust
// DANGEROUS: SQL Injection vulnerability
let query = format!(
    "SELECT * FROM users WHERE name = '{}'", 
    user_input  // What if user_input is: ' OR '1'='1
);
```

If `user_input` is `' OR '1'='1`, the query becomes:

```sql
SELECT * FROM users WHERE name = '' OR '1'='1'
```

This returns ALL users, bypassing the intended filter.

### Attack Examples

| Attack | Payload | Result |
|--------|---------|--------|
| Data exfiltration | `' UNION SELECT password FROM users--` | Leaks passwords |
| Bypass authentication | `' OR '1'='1` | Returns all rows |
| Delete data | `'; DROP TABLE users;--` | Destroys table |
| Read files | `' UNION SELECT load_extension('...` | System compromise |

## Defense Layer 1: Parameterized Queries

**Always use parameterized queries** for any user-controlled values:

```rust
// SAFE: Parameterized query
let users = sqlx::query_as::<_, User>(
    "SELECT * FROM users WHERE name = ?"
)
.bind(&user_input)  // Value is escaped/handled by the driver
.fetch_all(&pool)
.await?;
```

The database driver handles escaping—the user input can never become SQL code.

### When to Use Parameters

```rust
// ✅ SAFE: Values as parameters
sqlx::query("SELECT * FROM users WHERE id = ?")
    .bind(user_id)

sqlx::query("SELECT * FROM orders WHERE date > ? AND status = ?")
    .bind(start_date)
    .bind(status)

// ❌ UNSAFE: String formatting
format!("SELECT * FROM users WHERE id = {}", user_id)
format!("SELECT * FROM {} WHERE id = ?", table_name)  // Table names can't be parameterized!
```

### The Table Name Problem

You **cannot** parameterize table or column names:

```rust
// This WON'T work - table names can't be parameters
sqlx::query("SELECT * FROM ? WHERE id = ?")
    .bind(table_name)  // Error! 
    .bind(id)
```

For dynamic table/column names, use **allowlisting** (see Layer 2).

## Defense Layer 2: Allowlisting

When you can't use parameters (table names, column names, ORDER BY), use strict allowlists:

```rust
/// Tables that users are allowed to query
const ALLOWED_TABLES: &[&str] = &[
    "customers",
    "orders", 
    "products",
    "invoices",
];

/// Validate a table name against the allowlist
fn validate_table(table: &str) -> Result<&str> {
    let table_lower = table.to_lowercase();
    
    ALLOWED_TABLES
        .iter()
        .find(|&&t| t == table_lower)
        .map(|&t| t)
        .ok_or_else(|| anyhow!("Table '{}' is not accessible", table))
}

// Usage
let table = validate_table(&input.table)?;
let query = format!("SELECT * FROM {} WHERE id = ?", table);
```

### Column Name Allowlisting

```rust
fn validate_order_column(table: &str, column: &str) -> Result<&'static str> {
    let allowed = match table {
        "customers" => &["id", "name", "email", "created_at"][..],
        "orders" => &["id", "customer_id", "total", "order_date"][..],
        "products" => &["id", "name", "price", "category"][..],
        _ => return Err(anyhow!("Unknown table")),
    };
    
    allowed
        .iter()
        .find(|&&c| c == column.to_lowercase())
        .copied()
        .ok_or_else(|| anyhow!("Cannot sort by '{}'", column))
}

// Usage in ORDER BY
let order_col = validate_order_column("customers", &input.sort_by)?;
let query = format!(
    "SELECT * FROM customers ORDER BY {} {}",
    order_col,
    if input.ascending { "ASC" } else { "DESC" }
);
```

## Defense Layer 3: Query Validation

For MCP servers that accept raw SQL (like our `query` tool), validate the query structure:

```rust
/// Validate that a query is safe to execute
fn validate_query(sql: &str) -> Result<()> {
    let sql_upper = sql.trim().to_uppercase();
    
    // Must start with SELECT
    if !sql_upper.starts_with("SELECT") {
        return Err(anyhow!("Only SELECT queries are allowed"));
    }
    
    // Block dangerous keywords
    let blocked = [
        "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER",
        "TRUNCATE", "EXEC", "EXECUTE", "GRANT", "REVOKE",
        "INTO OUTFILE", "INTO DUMPFILE", "LOAD_FILE",
    ];
    
    for keyword in blocked {
        if sql_upper.contains(keyword) {
            return Err(anyhow!("'{}' is not allowed in queries", keyword));
        }
    }
    
    // Block multiple statements
    if sql.contains(';') {
        let parts: Vec<_> = sql.split(';').filter(|s| !s.trim().is_empty()).collect();
        if parts.len() > 1 {
            return Err(anyhow!("Multiple statements are not allowed"));
        }
    }
    
    // Block comments (often used in injection attacks)
    if sql.contains("--") || sql.contains("/*") {
        return Err(anyhow!("SQL comments are not allowed"));
    }
    
    Ok(())
}
```

### Limitations of Query Validation

Query validation is a **defense in depth** measure, not a primary defense:

```rust
// These attacks might bypass simple validation:

// Unicode tricks
"SELECT * FROM users WHERE name = 'admin'--" // Normal
"SELECT * FROM users WHERE name = 'admin'－－" // Unicode dash

// Case variations
"sElEcT * fRoM users" // Mixed case

// Encoded characters
"SELECT%20*%20FROM%20users" // URL encoded

// Comments
"SELECT/**/*/**/FROM/**/users" // Block comments
```

**Never rely on query validation alone.** Use it alongside:
1. Database user with minimal privileges
2. Row limits
3. Query timeouts
4. Audit logging

## Defense Layer 4: Database Permissions

The MCP server's database user should have **minimal privileges**:

```sql
-- Create a read-only user for the MCP server
CREATE USER 'mcp_reader'@'localhost' IDENTIFIED BY 'secure_password';

-- Grant only SELECT on specific tables
GRANT SELECT ON mydb.customers TO 'mcp_reader'@'localhost';
GRANT SELECT ON mydb.orders TO 'mcp_reader'@'localhost';
GRANT SELECT ON mydb.products TO 'mcp_reader'@'localhost';

-- Explicitly deny dangerous operations
-- (Usually not needed if you only GRANT SELECT, but good practice)
REVOKE ALL PRIVILEGES ON mydb.* FROM 'mcp_reader'@'localhost';
GRANT SELECT ON mydb.customers, mydb.orders, mydb.products TO 'mcp_reader'@'localhost';
```

For SQLite, use a read-only connection:

```rust
let pool = SqlitePoolOptions::new()
    .connect("sqlite:./data.db?mode=ro")  // Read-only mode
    .await?;
```

## Defense Layer 5: Query Timeouts

Prevent denial-of-service via expensive queries:

```rust
use tokio::time::{timeout, Duration};

async fn execute_with_timeout(
    pool: &DbPool,
    query: &str,
    max_duration: Duration,
) -> Result<Vec<SqliteRow>> {
    timeout(max_duration, async {
        sqlx::query(query)
            .fetch_all(pool.as_ref())
            .await
    })
    .await
    .map_err(|_| anyhow!("Query timed out after {:?}", max_duration))?
    .map_err(|e| anyhow!("Query failed: {}", e))
}

// Usage
let rows = execute_with_timeout(
    &pool, 
    &query, 
    Duration::from_secs(30)
).await?;
```

## Defense Layer 6: Result Limits

Always limit result sizes to prevent memory exhaustion:

```rust
const MAX_ROWS: i32 = 10_000;
const DEFAULT_ROWS: i32 = 100;

fn apply_limit(query: &str, requested_limit: Option<i32>) -> String {
    let limit = requested_limit
        .unwrap_or(DEFAULT_ROWS)
        .min(MAX_ROWS);
    
    let query_upper = query.to_uppercase();
    
    if query_upper.contains("LIMIT") {
        // Already has LIMIT - don't add another
        // But we should validate the existing limit isn't too high
        query.to_string()
    } else {
        format!("{} LIMIT {}", query.trim_end_matches(';'), limit)
    }
}
```

## Defense Layer 7: Audit Logging

Log all queries for security monitoring:

```rust
use tracing::{info, warn};

async fn execute_query(
    pool: &DbPool,
    query: &str,
    user_id: &str,
) -> Result<QueryOutput> {
    let start = std::time::Instant::now();
    
    // Log the query attempt
    info!(
        user_id = %user_id,
        query_preview = %query.chars().take(100).collect::<String>(),
        "Query execution started"
    );
    
    let result = sqlx::query(query)
        .fetch_all(pool.as_ref())
        .await;
    
    let duration = start.elapsed();
    
    match &result {
        Ok(rows) => {
            info!(
                user_id = %user_id,
                row_count = rows.len(),
                duration_ms = duration.as_millis(),
                "Query completed successfully"
            );
        }
        Err(e) => {
            warn!(
                user_id = %user_id,
                error = %e,
                duration_ms = duration.as_millis(),
                "Query failed"
            );
        }
    }
    
    // Convert result...
    Ok(result?)
}
```

## Complete Secure Query Implementation

Here's a production-ready query tool with all defenses:

```rust
use anyhow::{Result, anyhow};
use tokio::time::{timeout, Duration};
use tracing::{info, warn};

const MAX_ROWS: i32 = 10_000;
const DEFAULT_ROWS: i32 = 100;
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

const BLOCKED_KEYWORDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER",
    "TRUNCATE", "EXEC", "EXECUTE", "GRANT", "REVOKE",
    "INTO OUTFILE", "INTO DUMPFILE", "LOAD_FILE",
];

pub async fn secure_query(
    pool: &DbPool,
    input: QueryInput,
    user_context: &UserContext,
) -> Result<QueryOutput> {
    // Layer 3: Query validation
    validate_query(&input.query)?;
    
    // Layer 6: Apply row limit
    let limit = input.limit.unwrap_or(DEFAULT_ROWS).min(MAX_ROWS);
    let limited_query = apply_limit(&input.query, limit);
    
    // Layer 7: Audit logging
    info!(
        user_id = %user_context.user_id,
        query = %limited_query,
        "Executing query"
    );
    
    // Layer 5: Timeout
    let result = timeout(QUERY_TIMEOUT, async {
        sqlx::query(&limited_query)
            .fetch_all(pool.as_ref())
            .await
    })
    .await
    .map_err(|_| anyhow!("Query timed out"))?
    .map_err(|e| anyhow!("Query failed: {}", e))?;
    
    // Check truncation
    let truncated = result.len() > limit as usize;
    let rows: Vec<_> = result.into_iter().take(limit as usize).collect();
    
    info!(
        user_id = %user_context.user_id,
        row_count = rows.len(),
        truncated = truncated,
        "Query completed"
    );
    
    Ok(format_output(rows, truncated))
}

fn validate_query(sql: &str) -> Result<()> {
    let sql_upper = sql.trim().to_uppercase();
    
    if !sql_upper.starts_with("SELECT") {
        return Err(anyhow!("Only SELECT queries are allowed"));
    }
    
    for keyword in BLOCKED_KEYWORDS {
        if sql_upper.contains(keyword) {
            return Err(anyhow!("'{}' is not allowed", keyword));
        }
    }
    
    if sql.matches(';').count() > 1 {
        return Err(anyhow!("Multiple statements not allowed"));
    }
    
    Ok(())
}

fn apply_limit(query: &str, limit: i32) -> String {
    if query.to_uppercase().contains("LIMIT") {
        query.to_string()
    } else {
        format!("{} LIMIT {}", query.trim_end_matches(';'), limit + 1)
    }
}
```

## Security Checklist

Before deploying your database MCP server:

| Layer | Check | Status |
|-------|-------|--------|
| **Parameterization** | All user values use `.bind()` | ☐ |
| **Allowlisting** | Table/column names validated against lists | ☐ |
| **Query Validation** | Dangerous keywords blocked | ☐ |
| **Permissions** | Database user has SELECT only | ☐ |
| **Timeouts** | Queries timeout after reasonable duration | ☐ |
| **Limits** | Result size is bounded | ☐ |
| **Logging** | All queries are logged with user context | ☐ |
| **Sensitive Data** | PII/secrets columns are filtered | ☐ |

## Common Mistakes to Avoid

### ❌ Blocklisting Instead of Allowlisting

```rust
// BAD: Trying to block known bad things
if !input.contains("DROP") && !input.contains("DELETE") {
    // Still vulnerable to: DrOp, DEL/**/ETE, etc.
}

// GOOD: Only allow known good things
if ALLOWED_TABLES.contains(&table) {
    // Secure - we control the list
}
```

### ❌ Trusting Client-Side Validation

```rust
// BAD: Assuming the schema validation caught everything
// JsonSchema regex can be bypassed by determined attackers
#[schemars(regex(pattern = r"^SELECT"))]
query: String,  // Don't rely on this alone!

// GOOD: Always validate server-side
fn validate_query(sql: &str) -> Result<()> {
    // Server-side validation that can't be bypassed
}
```

### ❌ Logging Sensitive Data

```rust
// BAD: Logging full query might expose sensitive filters
info!("Query: {}", query);  // Might contain: WHERE ssn = '123-45-6789'

// GOOD: Log query structure, not values
info!(
    query_type = "SELECT",
    tables = ?extract_tables(&query),
    "Query executed"
);
```

---

*Continue to [Resource-Based Data Patterns](./ch03-04-resources.md) →*
