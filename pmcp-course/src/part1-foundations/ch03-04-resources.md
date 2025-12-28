# Resource-Based Data Patterns

MCP offers two ways to expose data: **tools** and **resources**. Understanding when to use each is key to building intuitive database servers.

## Tools vs Resources: When to Use Each

| Aspect | Tools | Resources |
|--------|-------|-----------|
| **Nature** | Actions, operations, queries | Documentation, reference data, metadata |
| **Data** | Dynamic, user-specific | Static or slowly-changing |
| **Parameters** | Flexible input parameters | URI-based, limited parameters |
| **Use case** | "Do something" | "Read about something" |
| **Caching** | Usually not cached | Often cached |

### Use Resources For:
- **Database schema documentation** - Table structures, column descriptions
- **Reference data** - Country codes, status enums, category lists
- **Configuration** - Database settings, connection info
- **Metadata** - Relationships, indexes, constraints
- **Help/documentation** - Query examples, usage guides

### Use Tools For:
- **Data queries** - SELECT with filters, joins, aggregations
- **Entity lookups** - Finding customers, orders, products
- **Search** - Full-text search, fuzzy matching
- **Analytics** - Aggregations, reports, dashboards

## Why Not `db://customers/12345`?

You might think resources are good for entity lookups like `db://customers/12345`. But consider:

```
Resource approach:
  Claude: "I need customer 12345"
  → Read db://customers/12345
  → Returns one customer
  → Claude: "Now I need their orders"
  → Read db://customers/12345/orders
  → Returns orders
  → Claude: "What's their total spend?"
  → ??? No resource for aggregations

Tool approach:
  Claude: "I need customer 12345 with their order history and total spend"
  → query("SELECT c.*, SUM(o.total) as total_spend 
           FROM customers c 
           JOIN orders o ON c.id = o.customer_id 
           WHERE c.id = 12345
           GROUP BY c.id")
  → Returns everything in one call
```

**Tools are more flexible for data access.** Resources shine for metadata and documentation.

## Practical Resource Examples

### Example 1: Database Schema Resource

Expose the database schema as a readable resource that Claude can reference:

```rust
use pmcp::resource::{Resource, ResourceContent, ResourceInfo};

/// Database schema documentation as a resource
pub struct SchemaResource {
    pool: DbPool,
}

impl Resource for SchemaResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://schema".to_string(),
            name: "Database Schema".to_string(),
            description: Some(
                "Complete database schema with tables, columns, types, and relationships. \
                 Use this to understand the database structure before writing queries."
                    .to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let schema = self.build_schema_documentation().await?;
        Ok(ResourceContent::json(&schema)?)
    }
}

#[derive(Serialize)]
struct SchemaDocumentation {
    database_name: String,
    tables: Vec<TableDocumentation>,
    relationships: Vec<Relationship>,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct TableDocumentation {
    name: String,
    description: String,
    columns: Vec<ColumnDocumentation>,
    primary_key: Vec<String>,
    row_count: i64,
    example_query: String,
}

#[derive(Serialize)]
struct ColumnDocumentation {
    name: String,
    data_type: String,
    nullable: bool,
    description: String,  // Can be populated from comments or a separate config
}

#[derive(Serialize)]
struct Relationship {
    from_table: String,
    from_column: String,
    to_table: String,
    to_column: String,
    relationship_type: String,  // "one-to-many", "many-to-many", etc.
}

impl SchemaResource {
    async fn build_schema_documentation(&self) -> Result<SchemaDocumentation> {
        let tables = self.get_all_tables().await?;
        let relationships = self.get_foreign_keys().await?;
        
        Ok(SchemaDocumentation {
            database_name: "Chinook Music Store".to_string(),
            tables,
            relationships,
            notes: vec![
                "All timestamps are in UTC".to_string(),
                "Monetary values are in USD".to_string(),
                "Use JOINs on foreign key relationships for related data".to_string(),
            ],
        })
    }

    async fn get_foreign_keys(&self) -> Result<Vec<Relationship>> {
        // Query SQLite's foreign key info
        let mut relationships = Vec::new();
        
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table'"
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        for (table,) in tables {
            let fks = sqlx::query(&format!("PRAGMA foreign_key_list({})", table))
                .fetch_all(self.pool.as_ref())
                .await?;
            
            for fk in fks {
                relationships.push(Relationship {
                    from_table: table.clone(),
                    from_column: fk.get("from"),
                    to_table: fk.get("table"),
                    to_column: fk.get("to"),
                    relationship_type: "many-to-one".to_string(),
                });
            }
        }
        
        Ok(relationships)
    }
}
```

**How Claude uses this:**

```
User: "What tables are related to customers?"

Claude: [Reads db://schema resource]
        
Based on the schema, the customers table is related to:
- invoices (customers.CustomerId → invoices.CustomerId) - one-to-many
- Each customer can have multiple invoices

The invoices table connects to:
- invoice_items (invoices.InvoiceId → invoice_items.InvoiceId)
- Which connects to tracks for the actual purchased items
```

### Example 2: Table-Specific Schema Resource

Provide detailed documentation for each table:

```rust
/// Individual table documentation
pub struct TableSchemaResource {
    pool: DbPool,
}

impl Resource for TableSchemaResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri_template: "db://schema/{table_name}".to_string(),
            name: "Table Schema".to_string(),
            description: Some(
                "Detailed schema for a specific table including columns, \
                 types, constraints, and example queries.".to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, uri: &str) -> Result<ResourceContent> {
        let table_name = uri.strip_prefix("db://schema/")
            .ok_or_else(|| anyhow!("Invalid URI"))?;
        
        // Validate table exists
        let valid_tables = self.get_table_names().await?;
        if !valid_tables.contains(&table_name.to_string()) {
            return Err(anyhow!("Table '{}' not found", table_name));
        }
        
        let doc = self.build_table_documentation(table_name).await?;
        Ok(ResourceContent::json(&doc)?)
    }
}

impl TableSchemaResource {
    async fn build_table_documentation(&self, table: &str) -> Result<TableDocumentation> {
        let columns = self.get_columns(table).await?;
        let pk = self.get_primary_key(table).await?;
        let row_count = self.get_row_count(table).await?;
        
        Ok(TableDocumentation {
            name: table.to_string(),
            description: self.get_table_description(table),
            columns,
            primary_key: pk,
            row_count,
            example_query: format!(
                "SELECT * FROM {} LIMIT 10", 
                table
            ),
        })
    }
    
    fn get_table_description(&self, table: &str) -> String {
        // In production, this might come from a config file or database comments
        match table {
            "customers" => "Customer information including contact details and location",
            "invoices" => "Sales transactions with date, customer, and billing info",
            "invoice_items" => "Line items for each invoice, linking to tracks",
            "tracks" => "Music tracks with duration, genre, and pricing",
            "albums" => "Music albums with artist reference",
            "artists" => "Music artists/bands",
            "genres" => "Music genre categories",
            "playlists" => "User-created playlists",
            "employees" => "Company employees with reporting structure",
            _ => "No description available",
        }.to_string()
    }
}
```

### Example 3: Reference Data Resources

Static lookup tables work well as resources:

```rust
/// Reference data: All available genres
pub struct GenresResource {
    pool: DbPool,
}

impl Resource for GenresResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://reference/genres".to_string(),
            name: "Music Genres".to_string(),
            description: Some(
                "List of all music genres in the database. \
                 Use these values when filtering tracks by genre.".to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let genres: Vec<Genre> = sqlx::query_as(
            "SELECT GenreId, Name FROM genres ORDER BY Name"
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(ResourceContent::json(&genres)?)
    }
}

/// Reference data: All media types
pub struct MediaTypesResource {
    pool: DbPool,
}

impl Resource for MediaTypesResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://reference/media-types".to_string(),
            name: "Media Types".to_string(),
            description: Some(
                "Available media formats (MP3, AAC, etc.). \
                 Use when filtering or understanding track formats.".to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let types: Vec<MediaType> = sqlx::query_as(
            "SELECT MediaTypeId, Name FROM media_types ORDER BY Name"
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(ResourceContent::json(&types)?)
    }
}
```

### Example 4: Query Examples Resource

Help Claude write better queries:

```rust
/// Example queries for common operations
pub struct QueryExamplesResource;

impl Resource for QueryExamplesResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://help/query-examples".to_string(),
            name: "Query Examples".to_string(),
            description: Some(
                "Example SQL queries for common operations. \
                 Reference these patterns when writing queries.".to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let examples = vec![
            QueryExample {
                name: "Customer with orders",
                description: "Get a customer and their order history",
                sql: r#"
                    SELECT c.FirstName, c.LastName, c.Email,
                           i.InvoiceId, i.InvoiceDate, i.Total
                    FROM customers c
                    JOIN invoices i ON c.CustomerId = i.CustomerId
                    WHERE c.CustomerId = ?
                    ORDER BY i.InvoiceDate DESC
                "#.to_string(),
            },
            QueryExample {
                name: "Top selling tracks",
                description: "Tracks ordered by number of sales",
                sql: r#"
                    SELECT t.Name as Track, ar.Name as Artist, 
                           COUNT(*) as TimesSold
                    FROM tracks t
                    JOIN invoice_items ii ON t.TrackId = ii.TrackId
                    JOIN albums al ON t.AlbumId = al.AlbumId
                    JOIN artists ar ON al.ArtistId = ar.ArtistId
                    GROUP BY t.TrackId
                    ORDER BY TimesSold DESC
                    LIMIT 10
                "#.to_string(),
            },
            QueryExample {
                name: "Revenue by country",
                description: "Total sales grouped by customer country",
                sql: r#"
                    SELECT c.Country, 
                           COUNT(DISTINCT c.CustomerId) as Customers,
                           SUM(i.Total) as Revenue
                    FROM customers c
                    JOIN invoices i ON c.CustomerId = i.CustomerId
                    GROUP BY c.Country
                    ORDER BY Revenue DESC
                "#.to_string(),
            },
            QueryExample {
                name: "Genre popularity",
                description: "Number of tracks per genre",
                sql: r#"
                    SELECT g.Name as Genre, COUNT(*) as TrackCount
                    FROM genres g
                    JOIN tracks t ON g.GenreId = t.GenreId
                    GROUP BY g.GenreId
                    ORDER BY TrackCount DESC
                "#.to_string(),
            },
        ];
        
        Ok(ResourceContent::json(&examples)?)
    }
}

#[derive(Serialize)]
struct QueryExample {
    name: &'static str,
    description: &'static str,
    sql: String,
}
```

### Example 5: Loading Resources from Files

Not all documentation comes from developers. DBAs, data analysts, and domain experts often maintain documentation in markdown or text files. Loading resources from the filesystem lets non-developers contribute without touching Rust code.

**Directory structure:**

```
db-explorer/
├── src/
│   └── main.rs
├── docs/                          # Maintained by DBAs/analysts
│   ├── database-guide.md
│   ├── tables/
│   │   ├── customers.md
│   │   ├── invoices.md
│   │   └── tracks.md
│   └── query-patterns.md
└── Cargo.toml
```

**Example markdown file (`docs/tables/customers.md`):**

```markdown
# Customers Table

The customers table stores contact information for all registered customers.

## Columns

| Column | Type | Description |
|--------|------|-------------|
| CustomerId | INTEGER | Primary key, auto-increment |
| FirstName | TEXT | Customer's first name (required) |
| LastName | TEXT | Customer's last name (required) |
| Email | TEXT | Unique email address (required) |
| Company | TEXT | Company name (optional) |
| Phone | TEXT | Contact phone number |
| Country | TEXT | Billing country |

## Common Queries

Find customers by country:
```sql
SELECT * FROM customers WHERE Country = 'USA' ORDER BY LastName;
```

Find customers with their total spend:
```sql
SELECT c.FirstName, c.LastName, SUM(i.Total) as TotalSpend
FROM customers c
JOIN invoices i ON c.CustomerId = i.CustomerId
GROUP BY c.CustomerId
ORDER BY TotalSpend DESC;
```

## Business Rules

- Email must be unique across all customers
- All monetary values are stored in USD
- Customer deletion is soft-delete only (sets DeletedAt timestamp)
```

**Loading markdown files as resources:**

```rust
use std::path::{Path, PathBuf};
use tokio::fs;

/// Documentation loaded from markdown files
pub struct FileDocumentationResource {
    docs_dir: PathBuf,
}

impl FileDocumentationResource {
    pub fn new(docs_dir: impl AsRef<Path>) -> Self {
        Self {
            docs_dir: docs_dir.as_ref().to_path_buf(),
        }
    }
}

impl Resource for FileDocumentationResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://docs/tables/{table_name}".to_string(),
            name: "Table Documentation".to_string(),
            description: Some(
                "Human-written documentation for database tables. \
                 Includes column descriptions, business rules, and example queries. \
                 Maintained by DBAs and data analysts.".to_string()
            ),
            mime_type: Some("text/markdown".to_string()),
        }
    }

    async fn read(&self, uri: &str) -> Result<ResourceContent> {
        let table_name = uri.strip_prefix("db://docs/tables/")
            .ok_or_else(|| anyhow!("Invalid URI format"))?;
        
        // Prevent path traversal attacks
        if table_name.contains("..") || table_name.contains('/') {
            return Err(anyhow!("Invalid table name"));
        }
        
        let file_path = self.docs_dir
            .join("tables")
            .join(format!("{}.md", table_name));
        
        // Check file exists within docs directory
        let canonical = file_path.canonicalize()
            .map_err(|_| anyhow!("Documentation not found for table '{}'", table_name))?;
        
        if !canonical.starts_with(self.docs_dir.canonicalize()?) {
            return Err(anyhow!("Invalid path"));
        }
        
        let content = fs::read_to_string(&file_path).await
            .map_err(|_| anyhow!("Documentation not found for table '{}'", table_name))?;
        
        Ok(ResourceContent::text(content))
    }
}

/// Database guide - single file resource
pub struct DatabaseGuideResource {
    docs_dir: PathBuf,
}

impl Resource for DatabaseGuideResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://docs/guide".to_string(),
            name: "Database Guide".to_string(),
            description: Some(
                "Comprehensive database guide written by the DBA team. \
                 Includes naming conventions, relationships, and best practices.".to_string()
            ),
            mime_type: Some("text/markdown".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let file_path = self.docs_dir.join("database-guide.md");
        let content = fs::read_to_string(&file_path).await
            .map_err(|_| anyhow!("Database guide not found"))?;
        
        Ok(ResourceContent::text(content))
    }
}
```

**Listing available documentation files:**

```rust
/// List all available table documentation
pub struct TableDocsListResource {
    docs_dir: PathBuf,
}

impl Resource for TableDocsListResource {
    fn info(&self) -> ResourceInfo {
        ResourceInfo {
            uri: "db://docs/tables".to_string(),
            name: "Available Table Documentation".to_string(),
            description: Some(
                "Lists all tables that have documentation available.".to_string()
            ),
            mime_type: Some("application/json".to_string()),
        }
    }

    async fn read(&self, _uri: &str) -> Result<ResourceContent> {
        let tables_dir = self.docs_dir.join("tables");
        let mut entries = fs::read_dir(&tables_dir).await?;
        
        let mut tables = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                if let Some(stem) = path.file_stem() {
                    tables.push(stem.to_string_lossy().to_string());
                }
            }
        }
        
        tables.sort();
        
        Ok(ResourceContent::json(&serde_json::json!({
            "tables": tables,
            "note": "Use db://docs/tables/{name} to read specific documentation"
        }))?)
    }
}
```

**Why file-based resources?**

| Approach | Best For |
|----------|----------|
| **Rust code** (hardcoded) | Static strings, compile-time constants |
| **Database queries** | Dynamic data, schema introspection |
| **File system** | Human-maintained docs, external contributions |

**Benefits of file-based documentation:**

1. **Non-developer contributions** - DBAs edit markdown, not Rust
2. **Version control** - Documentation changes tracked in git
3. **No recompilation** - Update docs without rebuilding
4. **Rich formatting** - Markdown supports tables, code blocks, links
5. **External tools** - Documentation can be generated by other tools

**Hot reloading pattern:**

For development, you might want to reload documentation without restarting:

```rust
impl Resource for FileDocumentationResource {
    fn cache_hint(&self) -> Option<Duration> {
        // In development: no caching, always fresh
        #[cfg(debug_assertions)]
        return None;
        
        // In production: cache for 5 minutes
        #[cfg(not(debug_assertions))]
        return Some(Duration::from_secs(300));
    }
}
```

## Registering Resources

Add resources alongside your tools:

```rust
let docs_dir = PathBuf::from("./docs");

let server = ServerBuilder::new("db-explorer", "1.0.0")
    .capabilities(ServerCapabilities {
        tools: Some(ToolCapabilities::default()),
        resources: Some(ResourceCapabilities::default()),
        ..Default::default()
    })
    // Tools for dynamic queries
    .tool(ListTables::new(pool.clone()).into_tool())
    .tool(Query::new(pool.clone()).into_tool())
    // Resources from database introspection
    .resource(SchemaResource::new(pool.clone()))
    .resource(TableSchemaResource::new(pool.clone()))
    .resource(GenresResource::new(pool.clone()))
    .resource(MediaTypesResource::new(pool.clone()))
    // Resources from code
    .resource(QueryExamplesResource)
    // Resources from filesystem (maintained by DBAs)
    .resource(DatabaseGuideResource::new(docs_dir.clone()))
    .resource(TableDocsListResource::new(docs_dir.clone()))
    .resource(FileDocumentationResource::new(docs_dir))
    .build()?;
```

## How Claude Uses Resources

When Claude connects to your server, it discovers available resources:

```
Available Resources:
- db://schema - Complete database schema
- db://schema/{table_name} - Schema for specific table
- db://reference/genres - Music genre list
- db://reference/media-types - Media format list
- db://help/query-examples - Example SQL queries
- db://docs/guide - Database guide (from file)
- db://docs/tables - List of documented tables
- db://docs/tables/{table_name} - Table documentation (from file)
```

Claude's workflow:

```
User: "What genres of music are in the database?"

Claude thinking:
  - This is asking about reference data
  - I can read db://reference/genres
  - No need to write a query

Claude: [Reads db://reference/genres]
        
The database contains 25 music genres:
Alternative, Blues, Classical, Comedy, Country...
```

```
User: "Show me the top 5 rock artists by sales"

Claude thinking:
  - I need to write a query
  - Let me check db://schema for table structure
  - And db://help/query-examples for patterns

Claude: [Reads db://schema]
        [Reads db://help/query-examples]
        [Uses query tool with adapted SQL]
```

## Benefits of This Pattern

### 1. Better AI Understanding

Resources give Claude context without requiring queries:

```
Without resources:
  Claude must guess table/column names or call list_tables first

With resources:
  Claude reads schema once, understands the entire database
```

### 2. Reduced Tool Calls

```
Without resources:
  1. list_tables() - What tables exist?
  2. query("PRAGMA table_info(customers)") - What columns?
  3. query("PRAGMA foreign_key_list(customers)") - Relationships?
  4. query("SELECT...") - Finally, the actual query

With resources:
  1. Read db://schema - Understand everything
  2. query("SELECT...") - Execute the query
```

### 3. Cacheable Documentation

Resources can be cached since they change infrequently:

```rust
impl Resource for SchemaResource {
    fn cache_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(300))  // Cache for 5 minutes
    }
}
```

### 4. Clear Separation of Concerns

| Resource | Purpose |
|----------|---------|
| `db://schema` | Understand the database |
| `db://reference/*` | Lookup valid values |
| `db://help/*` | Learn query patterns |

| Tool | Purpose |
|------|---------|
| `query` | Execute any SELECT |
| `list_tables` | Quick table overview |

## Summary

### When to Use Each Approach

| Data Type | Approach | Example |
|-----------|----------|---------|
| Table structures | **Resource** | `db://schema` |
| Column descriptions | **Resource** | `db://schema/customers` |
| Lookup tables (genres, countries) | **Resource** | `db://reference/genres` |
| Foreign key relationships | **Resource** | Part of `db://schema` |
| Query patterns/examples | **Resource** | `db://help/query-examples` |
| Human-written docs | **Resource** | `db://docs/tables/customers` |
| Entity data (customers, orders) | **Tool** | `query("SELECT...")` |
| Aggregations (totals, counts) | **Tool** | `query("SELECT SUM...")` |
| Search/filtering | **Tool** | `query("SELECT...WHERE...")` |

### Three Ways to Populate Resources

| Source | Best For | Example |
|--------|----------|---------|
| **Database queries** | Dynamic schema, reference tables | `db://schema`, `db://reference/genres` |
| **Rust code** | Static content, computed examples | `db://help/query-examples` |
| **Filesystem** | Human-maintained docs, external tools | `db://docs/tables/{name}` |

**Resources = Documentation. Tools = Operations.**

---

*Continue to [Handling Large Results](./ch03-05-pagination.md) →*
