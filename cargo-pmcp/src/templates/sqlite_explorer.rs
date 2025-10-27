//! SQLite Explorer template with workflow prompts
//!
//! Demonstrates:
//! - Database tools (execute_query, list_tables, get_sample_rows)
//! - Resources (schema discovery)
//! - Workflow prompts (simple, resource+tool, multi-step with bindings)
//! - Safe SQL execution with prepared statements

pub const SQLITE_EXPLORER_LIB: &str = r####"//! SQLite Explorer MCP Server
//!
//! Demonstrates all three MCP capabilities with database operations:
//! - Tools: Execute queries, list tables, get samples
//! - Resources: Database and table schemas
//! - Workflow Prompts: Multi-step database workflows with bindings

use pmcp::{
    Error, ResourceCollection, Result, Server, StaticResource, TypedTool,
};
use pmcp::server::workflow::{
    dsl::{constant, field, from_step, prompt_arg},
    SequentialWorkflow, ToolHandle, WorkflowStep,
};
use pmcp::types::{ServerCapabilities, ToolCapabilities, PromptCapabilities, ResourceCapabilities};
use rusqlite::{Connection, params_from_iter, OpenFlags};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

// ============================================================================
// CONFIGURATION
// ============================================================================

const MAX_ROWS: usize = 100;
const DATABASE_PATH: &str = "./chinook.db";

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ExecuteQueryInput {
    /// SQL query to execute (SELECT only)
    #[schemars(description = "SQL SELECT query to execute")]
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GetSampleRowsInput {
    /// Table name
    #[schemars(description = "Name of the table to sample")]
    pub table: String,

    /// Number of rows to return (default 5, max 20)
    #[schemars(description = "Number of sample rows (1-20)")]
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    5
}

// ============================================================================
// DATABASE HELPER FUNCTIONS
// ============================================================================

fn open_db() -> Result<Connection> {
    Connection::open_with_flags(
        DATABASE_PATH,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).map_err(|e| Error::internal(format!("Failed to open database: {}", e)))
}

fn validate_sql(sql: &str) -> Result<()> {
    let sql_upper = sql.trim().to_uppercase();

    // Must start with SELECT
    if !sql_upper.starts_with("SELECT") {
        return Err(Error::validation("Only SELECT queries are allowed"));
    }

    // Reject dangerous keywords
    let dangerous = ["INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "PRAGMA"];
    for keyword in &dangerous {
        if sql_upper.contains(keyword) {
            return Err(Error::validation(format!(
                "Query contains forbidden keyword: {}",
                keyword
            )));
        }
    }

    Ok(())
}

fn validate_table_name(conn: &Connection, table: &str) -> Result<()> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
        .map_err(|e| Error::internal(format!("Failed to prepare statement: {}", e)))?;

    let exists = stmt.exists([table])
        .map_err(|e| Error::internal(format!("Failed to check table: {}", e)))?;

    if !exists {
        return Err(Error::validation(format!("Table '{}' does not exist", table)));
    }

    Ok(())
}

// ============================================================================
// TOOL IMPLEMENTATIONS
// ============================================================================

async fn execute_query_tool(input: ExecuteQueryInput, _extra: pmcp::RequestHandlerExtra) -> Result<Value> {
    // Validate SQL
    validate_sql(&input.sql)?;

    // Open database
    let conn = open_db()?;

    // Prepare and execute query
    let mut stmt = conn.prepare(&input.sql)
        .map_err(|e| Error::validation(format!("Invalid SQL: {}", e)))?;

    // Get column names
    let column_names: Vec<String> = stmt.column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Execute query and collect rows
    let rows = stmt.query_map([], |row| {
        let mut row_data = serde_json::Map::new();
        for (i, col_name) in column_names.iter().enumerate() {
            let value: Value = match row.get_ref(i) {
                Ok(val) => match val {
                    rusqlite::types::ValueRef::Null => Value::Null,
                    rusqlite::types::ValueRef::Integer(i) => json!(i),
                    rusqlite::types::ValueRef::Real(f) => json!(f),
                    rusqlite::types::ValueRef::Text(s) => {
                        json!(String::from_utf8_lossy(s))
                    },
                    rusqlite::types::ValueRef::Blob(b) => {
                        json!(format!("<blob {} bytes>", b.len()))
                    },
                },
                Err(_) => Value::Null,
            };
            row_data.insert(col_name.clone(), value);
        }
        Ok(Value::Object(row_data))
    })
    .map_err(|e| Error::internal(format!("Query execution failed: {}", e)))?
    .take(MAX_ROWS)
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| Error::internal(format!("Failed to fetch rows: {}", e)))?;

    Ok(json!({
        "rows": rows,
        "row_count": rows.len(),
        "columns": column_names,
        "truncated": rows.len() >= MAX_ROWS
    }))
}

async fn list_tables_tool(_extra: pmcp::RequestHandlerExtra) -> Result<Value> {
    let conn = open_db()?;

    let mut stmt = conn.prepare(
        "SELECT name, type FROM sqlite_master WHERE type='table' ORDER BY name"
    ).map_err(|e| Error::internal(format!("Failed to list tables: {}", e)))?;

    let tables = stmt.query_map([], |row| {
        let name: String = row.get(0)?;

        // Get row count
        let count_query = format!("SELECT COUNT(*) FROM \"{}\"", name);
        let count: i64 = conn.query_row(&count_query, [], |r| r.get(0))
            .unwrap_or(0);

        Ok(json!({
            "name": name,
            "row_count": count
        }))
    })
    .map_err(|e| Error::internal(format!("Failed to query tables: {}", e)))?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| Error::internal(format!("Failed to collect tables: {}", e)))?;

    Ok(json!({
        "tables": tables,
        "table_count": tables.len()
    }))
}

async fn get_sample_rows_tool(input: GetSampleRowsInput, _extra: pmcp::RequestHandlerExtra) -> Result<Value> {
    let conn = open_db()?;

    // Validate table exists
    validate_table_name(&conn, &input.table)?;

    // Limit to max 20
    let limit = input.limit.min(20);

    // Use parameterized query (safe from injection)
    let query = format!("SELECT * FROM \"{}\" LIMIT ?", input.table);
    let mut stmt = conn.prepare(&query)
        .map_err(|e| Error::internal(format!("Failed to prepare query: {}", e)))?;

    // Get column names
    let column_names: Vec<String> = stmt.column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Execute and collect rows
    let rows = stmt.query_map([limit], |row| {
        let mut row_data = serde_json::Map::new();
        for (i, col_name) in column_names.iter().enumerate() {
            let value: Value = match row.get_ref(i) {
                Ok(val) => match val {
                    rusqlite::types::ValueRef::Null => Value::Null,
                    rusqlite::types::ValueRef::Integer(i) => json!(i),
                    rusqlite::types::ValueRef::Real(f) => json!(f),
                    rusqlite::types::ValueRef::Text(s) => {
                        json!(String::from_utf8_lossy(s))
                    },
                    rusqlite::types::ValueRef::Blob(b) => {
                        json!(format!("<blob {} bytes>", b.len()))
                    },
                },
                Err(_) => Value::Null,
            };
            row_data.insert(col_name.clone(), value);
        }
        Ok(Value::Object(row_data))
    })
    .map_err(|e| Error::internal(format!("Query execution failed: {}", e)))?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| Error::internal(format!("Failed to fetch rows: {}", e)))?;

    Ok(json!({
        "table": input.table,
        "rows": rows,
        "row_count": rows.len(),
        "columns": column_names
    }))
}

// ============================================================================
// RESOURCE IMPLEMENTATIONS
// ============================================================================

fn get_database_schema() -> Result<String> {
    let conn = open_db()?;

    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).map_err(|e| Error::internal(format!("Failed to get schema: {}", e)))?;

    let mut schema_md = String::from(r#"# Database Schema

## Tables

"#);

    let tables: Vec<(String, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })
    .map_err(|e| Error::internal(format!("Failed to query schema: {}", e)))?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| Error::internal(format!("Failed to collect schema: {}", e)))?;

    for (name, sql) in tables {
        schema_md.push_str("### ");
        schema_md.push_str(&name);
        schema_md.push_str("

```sql
");
        schema_md.push_str(&sql);
        schema_md.push_str("
```

");
    }

    Ok(schema_md)
}

fn get_table_schema(table_name: &str) -> Result<String> {
    let conn = open_db()?;

    // Validate table exists
    validate_table_name(&conn, table_name)?;

    // Get CREATE statement
    let create_sql: String = conn.query_row(
        r#"SELECT sql FROM sqlite_master WHERE type="table" AND name=?"#,
        [table_name],
        |row| row.get(0)
    ).map_err(|e| Error::internal(format!("Failed to get table schema: {}", e)))?;

    // Get column info
    let query = format!("PRAGMA table_info({})", table_name);
    let mut stmt = conn.prepare(&query)
        .map_err(|e| Error::internal(format!("Failed to get column info: {}", e)))?;

    let columns: Vec<Value> = stmt.query_map([], |row| {
        Ok(json!({
            "name": row.get::<_, String>(1)?,
            "type": row.get::<_, String>(2)?,
            "not_null": row.get::<_, i64>(3)? == 1,
            "default_value": row.get::<_, Option<String>>(4)?,
            "primary_key": row.get::<_, i64>(5)? == 1
        }))
    })
    .map_err(|e| Error::internal(format!("Failed to query columns: {}", e)))?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| Error::internal(format!("Failed to collect columns: {}", e)))?;

    // Get row count
    let count_query = format!("SELECT COUNT(*) FROM [{}]", table_name);
    let count: i64 = conn.query_row(
        &count_query,
        [],
        |row| row.get(0)
    ).unwrap_or(0);

    let mut schema_md = format!("# Table: {}

", table_name);
    schema_md.push_str(&format!("**Row count:** {}

", count));
    schema_md.push_str("## Schema

```sql
");
    schema_md.push_str(&create_sql);
    schema_md.push_str("
```

## Columns

");

    for col in &columns {
        if let Some(obj) = col.as_object() {
            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let type_name = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let not_null = obj.get("not_null").and_then(|v| v.as_bool()).unwrap_or(false);
            let pk = obj.get("primary_key").and_then(|v| v.as_bool()).unwrap_or(false);

            schema_md.push_str(&format!("- **{}** ({})", name, type_name));
            if pk {
                schema_md.push_str(" [PRIMARY KEY]");
            }
            if not_null {
                schema_md.push_str(" [NOT NULL]");
            }
            schema_md.push('\n');
        }
    }

    Ok(schema_md)
}

// ============================================================================
// WORKFLOW PROMPTS
// ============================================================================

fn create_monthly_sales_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "monthly_sales_report",
        "Generate sales report for a specific month with revenue and transaction metrics "
    )
    .argument("month", "Month number (1-12)", true)
    .argument("year", "Year (e.g., 2009)", true)
    .step(
        WorkflowStep::new("get_sales", ToolHandle::new("execute_query"))
            .arg("sql", constant(json!(
                r#"SELECT
                    strftime("%Y-%m", InvoiceDate) as Month,
                    COUNT(*) as TotalInvoices,
                    ROUND(SUM(Total), 2) as TotalRevenue,
                    ROUND(AVG(Total), 2) as AvgInvoiceValue,
                    MIN(Total) as MinInvoice,
                    MAX(Total) as MaxInvoice
                 FROM invoices
                 WHERE CAST(strftime("%m", InvoiceDate) AS INTEGER) = CAST(? AS INTEGER)
                   AND CAST(strftime("%Y", InvoiceDate) AS INTEGER) = CAST(? AS INTEGER)
                 GROUP BY Month "#
            )))
            .bind("sales_data")
    )
}

fn create_analyze_customer_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "analyze_customer",
        "Comprehensive customer analysis with purchase history and preferences "
    )
    .argument("customer_id", "Customer ID to analyze ", true)
    .step(
        WorkflowStep::new("get_customer", ToolHandle::new("execute_query"))
            .arg("sql", constant(json!(
                "SELECT CustomerId, FirstName, LastName, Email, Country, City
                 FROM customers
                 WHERE CustomerId = ?"
            )))
            .bind("customer_info")
    )
    .step(
        WorkflowStep::new("get_purchases", ToolHandle::new("execute_query"))
            .arg("sql", constant(json!(
                "SELECT
                    i.InvoiceId,
                    i.InvoiceDate,
                    COUNT(ii.InvoiceLineId) as ItemCount,
                    ROUND(SUM(ii.UnitPrice * ii.Quantity), 2) as Total
                 FROM invoices i
                 JOIN invoice_items ii ON i.InvoiceId = ii.InvoiceId
                 WHERE i.CustomerId = ?
                 GROUP BY i.InvoiceId
                 ORDER BY i.InvoiceDate DESC
                 LIMIT 10"
            )))
            .bind("purchase_history")
    )
    .step(
        WorkflowStep::new("get_lifetime_value", ToolHandle::new("execute_query"))
            .arg("sql", constant(json!(
                "SELECT
                    COUNT(DISTINCT i.InvoiceId) as TotalOrders,
                    ROUND(SUM(ii.UnitPrice * ii.Quantity), 2) as LifetimeValue,
                    ROUND(AVG(ii.UnitPrice * ii.Quantity), 2) as AvgOrderValue
                 FROM invoices i
                 JOIN invoice_items ii ON i.InvoiceId = ii.InvoiceId
                 WHERE i.CustomerId = ?"
            )))
            .bind("lifetime_metrics")
    )
}

fn create_top_tracks_customers_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new(
        "customers_who_bought_top_tracks",
        "Find customers who purchased top-selling tracks (multi-step workflow)"
    )
    .argument("limit", "Number of top tracks to find (default 5)", false)
    .step(
        WorkflowStep::new("get_top_tracks", ToolHandle::new("execute_query"))
            .arg("sql", constant(json!(
                "SELECT
                    t.TrackId,
                    t.Name as TrackName,
                    ar.Name as ArtistName,
                    al.Title as AlbumName,
                    COUNT(ii.InvoiceLineId) as TimesPurchased,
                    ROUND(SUM(ii.UnitPrice * ii.Quantity), 2) as TotalRevenue
                 FROM tracks t
                 JOIN invoice_items ii ON t.TrackId = ii.TrackId
                 JOIN albums al ON t.AlbumId = al.AlbumId
                 JOIN artists ar ON al.ArtistId = ar.ArtistId
                 GROUP BY t.TrackId
                 ORDER BY TimesPurchased DESC
                 LIMIT COALESCE(?, 5)"
            )))
            .bind("top_tracks")
    )
    // Note: The workflow deliberately stops here
    // The client's LLM must continue by:
    // 1. Extracting TrackIds from "top_tracks" binding
    // 2. Generating SQL to find customers who bought those tracks
    // 3. Calling execute_query with the generated SQL
}

/// Build the SQLite explorer server
pub fn build_sqlite_server() -> Result<Server> {
    // Create resources
    let mut resources = ResourceCollection::new();

    // Add database schema resource
    let schema = get_database_schema()?;
    resources = resources.add_resource(
        StaticResource::new_text("sqlite://schema", &schema)
            .with_name("Database Schema")
            .with_description("Complete database schema with all tables")
            .with_mime_type("text/markdown")
    );

    // Note: Table-specific schemas are added dynamically in a real implementation
    // For this template, we'll document the pattern

    Server::builder()
        .name("sqlite-explorer")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: Some(true) }),
            prompts: Some(PromptCapabilities { list_changed: Some(true) }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        })
        // Add tools
        .tool(
            "execute_query",
            TypedTool::new("execute_query", |input: ExecuteQueryInput, extra| {
                Box::pin(execute_query_tool(input, extra))
            })
            .with_description("Execute a SELECT query on the database (read-only)")
        )
        .tool(
            "list_tables",
            TypedTool::new("list_tables", |_input: (), extra| {
                Box::pin(list_tables_tool(extra))
            })
            .with_description("List all tables in the database with row counts")
        )
        .tool(
            "get_sample_rows",
            TypedTool::new("get_sample_rows", |input: GetSampleRowsInput, extra| {
                Box::pin(get_sample_rows_tool(input, extra))
            })
            .with_description("Get sample rows from a specific table")
        )
        // Add workflow prompts
        .prompt_workflow(create_monthly_sales_workflow())?
        .prompt_workflow(create_analyze_customer_workflow())?
        .prompt_workflow(create_top_tracks_customers_workflow())?
        // Add resources
        .resources(resources)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_validation() {
        // Valid queries
        assert!(validate_sql("SELECT * FROM customers").is_ok());
        assert!(validate_sql("  SELECT id FROM users WHERE active = 1").is_ok());

        // Invalid queries
        assert!(validate_sql("INSERT INTO customers VALUES (1)").is_err());
        assert!(validate_sql("UPDATE customers SET name = 'x'").is_err());
        assert!(validate_sql("DELETE FROM customers").is_err());
        assert!(validate_sql("DROP TABLE customers").is_err());
        assert!(validate_sql("SELECT * FROM customers; DROP TABLE users").is_err());
    }

    #[tokio::test]
    async fn test_server_builds() {
        // This will fail without chinook.db, but tests the API
        // let server = build_sqlite_server();
        // In real usage, ensure chinook.db is present
    }
}
"####;
