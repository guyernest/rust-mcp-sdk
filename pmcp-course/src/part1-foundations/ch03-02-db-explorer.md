# Building db-explorer

Let's build a database MCP server. Like Chapter 2, we'll start by getting a working server running in under 5 minutes—then we'll explore how it works.

## Try It First: Database Server in 5 Minutes

### Step 1: Add the Server

From your existing workspace (or create a new one with `cargo pmcp new`):

```bash
cargo pmcp add server db-explorer --template db-explorer
```

### Step 2: Get a Sample Database

We'll use the Chinook database—a sample music store with customers, invoices, artists, and tracks:

```bash
curl -L -o chinook.db https://github.com/lerocha/chinook-database/raw/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite
```

### Step 3: Run the Server

```bash
DATABASE_URL=sqlite:./chinook.db cargo pmcp dev db-explorer
```

You should see:

```
INFO db_explorer: Starting db-explorer server
INFO db_explorer: Database: sqlite:./chinook.db
INFO db_explorer: Connected to database
INFO server_common: Listening on http://0.0.0.0:3000
```

### Step 4: Connect to Claude Code

In a new terminal:

```bash
claude mcp add db-explorer -t http http://localhost:3000
```

### Step 5: Explore the Database!

Start Claude Code and try these prompts:

> **"What tables are in the database?"**

Claude will call `list_tables` and show you the schema:

```
The database contains 11 tables:
- albums (347 rows) - AlbumId, Title, ArtistId
- artists (275 rows) - ArtistId, Name
- customers (59 rows) - CustomerId, FirstName, LastName, Email...
- employees (8 rows) - EmployeeId, LastName, FirstName...
- genres (25 rows) - GenreId, Name
- invoices (412 rows) - InvoiceId, CustomerId, InvoiceDate...
- invoice_items (2240 rows) - ...
- media_types (5 rows) - ...
- playlists (18 rows) - ...
- playlist_track (8715 rows) - ...
- tracks (3503 rows) - ...
```

> **"Which country has the most customers?"**

Claude writes SQL and queries the database:

```sql
SELECT Country, COUNT(*) as customer_count 
FROM customers 
GROUP BY Country 
ORDER BY customer_count DESC 
LIMIT 5
```

> **"Show me the top 5 selling artists by total revenue"**

Claude handles the complex join:

```sql
SELECT ar.Name, SUM(ii.UnitPrice * ii.Quantity) as Revenue
FROM artists ar
JOIN albums al ON ar.ArtistId = al.ArtistId
JOIN tracks t ON al.AlbumId = t.AlbumId
JOIN invoice_items ii ON t.TrackId = ii.TrackId
GROUP BY ar.ArtistId
ORDER BY Revenue DESC
LIMIT 5
```

> **"What genres are most popular by number of tracks sold?"**

> **"Find customers who haven't made a purchase in the last year"**

> **"What's the average invoice total by country?"**

### What Just Happened?

You gave Claude direct access to a database. It can:

1. **Discover the schema** - Understand what data is available
2. **Write SQL** - Translate natural language to queries
3. **Execute safely** - Only SELECT queries are allowed
4. **Present results** - Format data for human understanding

This is the power of database MCP servers.

---

## Test with MCP Inspector

Before connecting to Claude, you can test your server interactively:

```bash
npx @modelcontextprotocol/inspector http://localhost:3000/mcp
```

This opens a web UI where you can:

| Action | How |
|--------|-----|
| Browse tools | See `list_tables` and `query` with their schemas |
| Call list_tables | Click the tool, then "Execute" (no parameters needed) |
| Run a query | Enter `{"query": "SELECT * FROM artists LIMIT 5"}` |
| See raw JSON | View the exact MCP protocol messages |

Try these queries in the inspector:

```json
{"query": "SELECT * FROM customers LIMIT 5"}
```

```json
{"query": "SELECT Country, COUNT(*) as count FROM customers GROUP BY Country"}
```

```json
{"query": "SELECT * FROM artists WHERE Name LIKE '%Rock%'"}
```

---

## How It Works

Now that you've seen it in action, let's understand the code. The db-explorer template creates this structure:

```
servers/db-explorer/
├── Cargo.toml
└── src/
    ├── main.rs           # Entry point, server setup
    ├── database.rs       # Connection pool management
    └── tools/
        ├── mod.rs        # Tool exports
        ├── list_tables.rs # Schema introspection
        └── query.rs      # SQL execution
```

### The Database Connection

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

**Key points:**
- `Arc<Pool<Sqlite>>` - Shared connection pool, thread-safe
- `max_connections(5)` - Limits concurrent database connections
- Pool is shared between all tool handlers

### The list_tables Tool

```rust
// src/tools/list_tables.rs (simplified)

#[derive(Debug, Serialize, JsonSchema)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: i64,
}

async fn list_tables_impl(pool: &DbPool) -> Result<Vec<TableInfo>> {
    // Get table names from SQLite's system catalog
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master 
         WHERE type = 'table' 
         AND name NOT LIKE 'sqlite_%'"
    )
    .fetch_all(pool.as_ref())
    .await?;

    // For each table, get columns and row count
    let mut result = Vec::new();
    for (table_name,) in tables {
        let columns = get_columns(pool, &table_name).await?;
        let row_count = get_row_count(pool, &table_name).await?;
        
        result.push(TableInfo { name: table_name, columns, row_count });
    }

    Ok(result)
}
```

**This tool:**
- Queries SQLite's `sqlite_master` for table names
- Uses `PRAGMA table_info()` to get column details
- Counts rows in each table
- Returns structured data Claude can understand

### The query Tool

```rust
// src/tools/query.rs (simplified)

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryInput {
    /// SQL query to execute (SELECT only)
    pub query: String,
    
    /// Maximum rows to return
    #[serde(default = "default_limit")]
    pub limit: i32,
}

async fn query_impl(pool: &DbPool, input: QueryInput) -> Result<QueryOutput> {
    // Security: Only allow SELECT
    if !input.query.trim().to_uppercase().starts_with("SELECT") {
        return Err(anyhow!("Only SELECT queries are allowed"));
    }

    // Security: Block dangerous keywords
    let blocked = ["INSERT", "UPDATE", "DELETE", "DROP"];
    for keyword in blocked {
        if input.query.to_uppercase().contains(keyword) {
            return Err(anyhow!("{} is not allowed", keyword));
        }
    }

    // Add LIMIT if not present
    let limited_query = if !input.query.to_uppercase().contains("LIMIT") {
        format!("{} LIMIT {}", input.query, input.limit + 1)
    } else {
        input.query.clone()
    };

    // Execute and return results
    let rows = sqlx::query(&limited_query)
        .fetch_all(pool.as_ref())
        .await?;

    Ok(format_results(rows, input.limit))
}
```

**Security measures:**
1. **SELECT only** - Rejects INSERT, UPDATE, DELETE
2. **Keyword blocking** - Extra protection against injection
3. **Automatic LIMIT** - Prevents memory exhaustion
4. **Truncation detection** - Tells Claude if more rows exist

### The Main Entry Point

```rust
// src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./chinook.db".to_string());

    // Create connection pool
    let pool = create_pool(&database_url).await?;

    // Build MCP server with both tools
    let server = ServerBuilder::new("db-explorer", "1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool(ListTables::new(pool.clone()).into_tool())
        .tool(Query::new(pool.clone()).into_tool())
        .build()?;

    // Start HTTP server
    server_common::create_http_server(server)
        .serve("0.0.0.0:3000")
        .await
}
```

---

## Building from Scratch

Want to build it yourself instead of using the template? Here's the complete process:

### 1. Create Minimal Server

```bash
cargo pmcp add server my-db-server --template minimal
```

### 2. Add Dependencies

Edit `servers/my-db-server/Cargo.toml`:

```toml
[dependencies]
pmcp = { path = "../../pmcp" }
server-common = { path = "../../server-common" }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 3. Create the Files

Create the file structure shown above, implementing:
- `database.rs` - Connection pool
- `tools/list_tables.rs` - Schema discovery
- `tools/query.rs` - SQL execution
- `tools/mod.rs` - Module exports
- `main.rs` - Server setup

The complete code for each file is in the [Chapter 3 Exercises](./ch03-exercises.md).

---

## What We Built

| Component | Purpose |
|-----------|---------|
| `DbPool` | Shared, pooled database connections |
| `list_tables` | Schema discovery for Claude |
| `query` | Flexible SQL execution with safety checks |
| Connection pooling | Efficient resource usage |
| Query validation | Basic SQL injection protection |
| Result limiting | Memory safety |

## Limitations of This Basic Server

This server works, but has security limitations:

| Issue | Risk | Solution |
|-------|------|----------|
| String-based validation | Can be bypassed | Proper parsing |
| No parameterized queries | SQL injection | Use `.bind()` |
| No authentication | Anyone can query | Add OAuth |
| No audit logging | No accountability | Log all queries |
| No column filtering | May expose PII | Allowlist columns |

The next sections address these:

1. **[SQL Safety](./ch03-03-sql-safety.md)** - Proper parameterized queries, defense in depth
2. **[Resources](./ch03-04-resources.md)** - Structured access patterns
3. **[Pagination](./ch03-05-pagination.md)** - Handling large result sets

> **Production Security Note**
>
> The examples in Part 1 focus on MCP fundamentals and omit authentication for simplicity. In production deployments, you should:
>
> 1. **Require OAuth authentication** for all MCP requests
> 2. **Pass access tokens through** to backend data systems as the source of truth for permissions
> 3. **Let the database enforce row-level security** based on the authenticated user
>
> See [Part 5: Security](../part5-security/ch13-oauth.md) for complete OAuth integration patterns with AWS Cognito, Auth0, and Microsoft Entra ID.

---

*Continue to [SQL Safety and Injection Prevention](./ch03-03-sql-safety.md) →*
