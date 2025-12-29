# Single Responsibility for Tools

The single responsibility principle for MCP tools isn't about code organization—it's about AI comprehension. A tool that does one thing well is a tool that gets used correctly.

## The Problem with Multi-Purpose Tools

Consider this "swiss army knife" tool:

```rust
Tool::new("data_operation")
    .description("Perform data operations - query, insert, update, delete, export, import, validate, transform")
    .input_schema(json!({
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["query", "insert", "update", "delete", "export", "import", "validate", "transform"]
            },
            "table": { "type": "string" },
            "data": { "type": "object" },
            "format": { "type": "string" },
            "options": { "type": "object" }
        }
    }))
```

What's wrong with this design?

### 1. AI Decision Paralysis

The AI must understand 8 different behaviors from one tool. When a user says "get me the sales data," the AI must reason:

```
User: "get me the sales data"

AI reasoning about data_operation:
- Is this a "query" operation?
- Or should I "export" to get the data?
- What's the difference between query and export here?
- The description doesn't clarify...
- Maybe I should ask the user?
```

### 2. Parameter Confusion

Different operations need different parameters, but they share one schema:

```rust
// For "query": table and maybe some filter options
// For "insert": table and data object
// For "export": table and format
// For "transform": data and transformation options

// All crammed into one ambiguous schema
{
    "table": "???",     // Required for some, ignored by others
    "data": "???",      // Sometimes input, sometimes not
    "format": "???",    // Only for export
    "options": "???"    // Means different things per operation
}
```

### 3. Error Messages Are Vague

When something goes wrong, what failed?

```json
{
    "error": "Invalid parameters for data_operation"
}
```

Did the query syntax fail? The data format? The export path? The tool is too broad to give useful feedback.

## Single Responsibility Refactoring

Split the swiss army knife into focused tools:

```rust
// READ operations
Tool::new("db_query")
    .description(
        "Execute read-only SQL queries. \
        Use for retrieving data from any table. \
        Returns results as JSON array."
    )
    .input_schema(json!({
        "required": ["sql"],
        "properties": {
            "sql": { "type": "string" },
            "limit": { "type": "integer", "default": 100 }
        }
    }))

// WRITE operations (separate from read for safety)
Tool::new("db_modify")
    .description(
        "Insert, update, or delete records. \
        Use when the user explicitly requests data changes. \
        Returns affected row count."
    )
    .input_schema(json!({
        "required": ["operation", "table"],
        "properties": {
            "operation": { "enum": ["insert", "update", "delete"] },
            "table": { "type": "string" },
            "data": { "type": "object" },
            "where": { "type": "string" }
        }
    }))

// EXPORT operations
Tool::new("db_export")
    .description(
        "Export table data to file formats (CSV, JSON, Parquet). \
        Use when user needs to download or share data. \
        Returns file path or download URL."
    )
    .input_schema(json!({
        "required": ["table", "format"],
        "properties": {
            "table": { "type": "string" },
            "format": { "enum": ["csv", "json", "parquet"] },
            "filter": { "type": "string" }
        }
    }))

// VALIDATION operations
Tool::new("db_validate")
    .description(
        "Check data integrity and validate against schemas. \
        Use before imports or to diagnose data issues. \
        Returns validation report."
    )
```

Now the AI's job is clear:
- User wants data? → `db_query`
- User wants to change data? → `db_modify`
- User wants a file? → `db_export`
- User wants to check data? → `db_validate`

## The "One Sentence" Rule

If you can't describe what a tool does in one clear sentence, it's doing too much:

```rust
// FAIL: Multiple responsibilities
"Perform data operations - query, insert, update, delete, export, import, validate, transform"

// PASS: Single responsibility
"Execute read-only SQL queries against the database"
"Export table data to file formats"
"Validate data integrity against schemas"
```

## Balancing Granularity

Single responsibility doesn't mean creating hundreds of micro-tools. Find the right level of abstraction:

### Too Granular (tool explosion)

```rust
Tool::new("select_from_customers")
Tool::new("select_from_orders")
Tool::new("select_from_products")
Tool::new("select_with_where")
Tool::new("select_with_join")
Tool::new("select_with_group_by")
// 50 more query variations...
```

### Too Coarse (swiss army knife)

```rust
Tool::new("database")  // Does everything database-related
```

### Just Right (task-oriented)

```rust
Tool::new("db_query")      // Read data with SQL
Tool::new("db_schema")     // Explore table structures
Tool::new("db_export")     // Export to files
Tool::new("db_admin")      // Administrative operations (with appropriate guards)
```

## Responsibility and Safety

Single responsibility also enables better safety controls:

```rust
// Read operations: safe, can be used freely
Tool::new("db_query")
    .description("Read-only queries - safe for exploration")

// Write operations: need confirmation
Tool::new("db_modify")
    .description("Modifies data - AI should confirm with user before destructive operations")

// Admin operations: restricted
Tool::new("db_admin")
    .description("Administrative operations - requires explicit user authorization")
    .annotations(json!({
        "requires_confirmation": true,
        "risk_level": "high"
    }))
```

With separate tools, you can apply different security policies to each.

## The Composition Principle

Single-responsibility tools compose better than multi-purpose tools:

```rust
// Multi-purpose tools can't be combined
Tool::new("analyze_and_report")  // Does analysis AND reporting
// What if user wants analysis without report? Too bad.

// Single-purpose tools compose flexibly
Tool::new("db_query")           // Get the data
Tool::new("data_analyze")       // Analyze it
Tool::new("report_generate")    // Create report

// AI can now:
// - Query without analysis
// - Analyze without report
// - Query, analyze, AND report
// - Any combination the user needs
```

## Testing Single Responsibility

### The "What If" Test

For each tool, ask: "What if the user only wants part of what this tool does?"

```rust
// FAIL: Can't partially use
Tool::new("fetch_and_format_data")
// What if user wants raw data without formatting?

// PASS: Separable concerns
Tool::new("fetch_data")
Tool::new("format_data")
```

### The "Who Cares" Test

For each operation in a tool, ask: "Would a different user care about just this operation?"

```rust
// In "data_operation":
// - query: Data analysts care about this
// - insert: Application developers care about this
// - export: Business users care about this
// - validate: Data engineers care about this

// Different audiences = different tools
```

### The "Change Impact" Test

If the tool's behavior needs to change, how much else breaks?

```rust
// Multi-purpose: changing export format affects everything
Tool::new("data_operation")  // Export format change touches all code paths

// Single-purpose: changes are isolated
Tool::new("db_export")  // Only export code needs to change
```

## Summary

Single responsibility for MCP tools means:

| Principle | Benefit |
|-----------|---------|
| One clear purpose per tool | AI selects correctly |
| Focused parameter schemas | Less confusion, better errors |
| Separable concerns | Users get exactly what they need |
| Composable operations | Flexible workflows |
| Isolated safety controls | Appropriate permissions per operation |

Remember: you're not writing code for other developers. You're writing tools for AI clients that must choose correctly from dozens of options. Make their job easy.
