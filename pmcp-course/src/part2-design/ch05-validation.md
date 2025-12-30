# Input Validation and Output Schemas

When an AI client calls your tool, it constructs the parameters based on your schema and description. Unlike human developers who read documentation carefully, AI clients make inferences—and sometimes those inferences are wrong.

Robust validation isn't just defensive programming. It's a critical feedback mechanism that helps AI clients learn and self-correct.

## The AI Parameter Problem

Consider what happens when an AI calls a database query tool:

```
User: "Show me orders from last month"

AI reasoning:
- Need to call sales_query tool
- Parameter "date_range" expects... what format?
- Description says "date range for filtering"
- I'll try: "last month"
```

```rust
// What the AI sends
{
    "tool": "sales_query",
    "parameters": {
        "query": "SELECT * FROM orders",
        "date_range": "last month"  // Natural language, not ISO dates
    }
}
```

Without proper validation, this might:
- Crash with a parse error
- Silently ignore the date_range
- Return all orders (no filtering)

With proper validation, the AI gets useful feedback:

```json
{
    "error": {
        "code": "INVALID_DATE_RANGE",
        "message": "date_range must be an object with 'start' and 'end' ISO 8601 dates",
        "expected": {
            "start": "2024-11-01",
            "end": "2024-11-30"
        },
        "received": "last month"
    }
}
```

The AI can now self-correct and retry with the proper format.

## Why Schemas Matter

MCP tools declare their parameters using JSON Schema. This serves multiple purposes:

### 1. Documentation for AI Clients

The schema tells the AI what parameters are valid:

```rust
Tool::new("sales_query")
    .input_schema(json!({
        "type": "object",
        "required": ["query"],
        "properties": {
            "query": {
                "type": "string",
                "description": "SQL SELECT query"
            },
            "date_range": {
                "type": "object",
                "description": "Filter results to this date range",
                "properties": {
                    "start": {
                        "type": "string",
                        "format": "date",
                        "description": "Start date (ISO 8601)"
                    },
                    "end": {
                        "type": "string",
                        "format": "date",
                        "description": "End date (ISO 8601)"
                    }
                }
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 10000,
                "default": 100
            }
        }
    }))
```

### 2. Pre-Call Validation

Many MCP clients validate parameters against the schema before sending the request. This catches obvious errors early.

### 3. Runtime Validation

Your server should also validate, because:
- Not all clients validate
- Schemas can't express all constraints
- Defense in depth is good practice

## The Validation Spectrum

Different levels of validation serve different purposes:

| Level | What It Catches | Example |
|-------|----------------|---------|
| **Schema** | Type mismatches | String instead of number |
| **Format** | Structural errors | Invalid date format |
| **Business** | Domain violations | Future dates for historical query |
| **Security** | Dangerous inputs | SQL injection attempts |

### The PMCP SDK Approach: TypedTool

The PMCP SDK provides `TypedTool` which uses Rust's type system to handle schema validation automatically. Define your input as a struct, and the SDK generates the JSON schema and validates inputs for you:

```rust
use pmcp::{TypedTool, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

/// Input parameters for sales queries
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SalesQueryInput {
    /// SQL SELECT query to execute against PostgreSQL 15.
    /// Supports CTEs, WINDOW functions, and JSON operators.
    query: String,

    /// Optional date range filter for the query
    date_range: Option<DateRange>,

    /// Maximum rows to return (1-10000, default: 100)
    #[serde(default = "default_limit")]
    limit: u32,

    /// Query timeout in milliseconds (100-30000, default: 5000)
    #[serde(default = "default_timeout")]
    timeout_ms: u32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DateRange {
    /// Start date in ISO 8601 format (YYYY-MM-DD)
    start: NaiveDate,
    /// End date in ISO 8601 format (YYYY-MM-DD)
    end: NaiveDate,
}

fn default_limit() -> u32 { 100 }
fn default_timeout() -> u32 { 5000 }
```

The `///` doc comments become field descriptions in the generated JSON schema. The AI sees:

```json
{
  "properties": {
    "query": {
      "type": "string",
      "description": "SQL SELECT query to execute against PostgreSQL 15. Supports CTEs, WINDOW functions, and JSON operators."
    },
    "date_range": {
      "type": "object",
      "description": "Optional date range filter for the query",
      "properties": {
        "start": { "type": "string", "format": "date", "description": "Start date in ISO 8601 format (YYYY-MM-DD)" },
        "end": { "type": "string", "format": "date", "description": "End date in ISO 8601 format (YYYY-MM-DD)" }
      }
    },
    "limit": {
      "type": "integer",
      "description": "Maximum rows to return (1-10000, default: 100)"
    }
  },
  "required": ["query"]
}
```

### Type-Safe Validation in the Handler

With `TypedTool`, schema validation happens automatically. Your handler receives a strongly-typed struct, and you add business and security validation:

```rust
let sales_query_tool = TypedTool::new(
    "sales_query",
    |args: SalesQueryInput, _extra| {
        Box::pin(async move {
            // 1. Schema validation: ALREADY DONE by TypedTool!
            //    - args.query is guaranteed to be a String
            //    - args.date_range, if present, has valid NaiveDate fields
            //    - Invalid JSON is rejected before this code runs

            // 2. Format validation: Partially handled by types
            //    - NaiveDate parsing validates ISO 8601 format
            //    - Add additional format checks as needed
            if args.query.trim().is_empty() {
                return Err(Error::Validation(
                    "Query cannot be empty".to_string()
                ));
            }

            // 3. Business validation
            if let Some(ref dr) = args.date_range {
                if dr.end < dr.start {
                    return Err(Error::Validation(
                        "date_range.end must be after date_range.start".to_string()
                    ));
                }
                if dr.end > chrono::Utc::now().date_naive() {
                    return Err(Error::Validation(
                        "Cannot query future dates".to_string()
                    ));
                }
            }

            // Enforce bounds even if client ignores schema hints
            let limit = args.limit.min(10000).max(1);
            let timeout = args.timeout_ms.min(30000).max(100);

            // 4. Security validation
            validate_sql_security(&args.query)?;

            // Execute with validated parameters
            execute_query(&args.query, args.date_range, limit, timeout).await
        })
    }
)
.with_description(
    "Execute read-only SQL queries against the sales PostgreSQL 15 database. \
    Returns results as JSON array. Use PostgreSQL date functions like DATE_TRUNC."
);

fn validate_sql_security(sql: &str) -> Result<(), Error> {
    let sql_upper = sql.to_uppercase();

    // Only allow SELECT queries
    if !sql_upper.trim_start().starts_with("SELECT") {
        return Err(Error::Validation(
            "Only SELECT queries are allowed".to_string()
        ));
    }

    // Block dangerous constructs
    let forbidden = ["DROP", "DELETE", "INSERT", "UPDATE", "TRUNCATE", "ALTER"];
    for keyword in forbidden {
        if sql_upper.contains(keyword) {
            return Err(Error::Validation(
                format!("{} operations are not permitted", keyword)
            ));
        }
    }

    Ok(())
}
```

### Using Enums for Constrained Values

When parameters have a fixed set of valid values, use Rust enums instead of validating strings:

```rust
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Csv,
    Markdown,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SalesRegion {
    NorthAmerica,
    Europe,
    AsiaPacific,
    LatinAmerica,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SalesReportInput {
    /// Sales region to report on
    region: SalesRegion,

    /// Output format for the report
    #[serde(default)]
    format: OutputFormat,

    /// Include year-over-year comparison
    #[serde(default)]
    include_yoy: bool,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Json
    }
}
```

The generated schema includes the valid enum values:

```json
{
  "properties": {
    "region": {
      "type": "string",
      "enum": ["north_america", "europe", "asia_pacific", "latin_america"],
      "description": "Sales region to report on"
    },
    "format": {
      "type": "string",
      "enum": ["json", "csv", "markdown"],
      "description": "Output format for the report"
    }
  }
}
```

The AI knows exactly which values are valid and won't try "JSON", "Json", or "application/json".

### Why TypedTool is Better

| Manual JSON Schema | TypedTool with Structs |
|--------------------|------------------------|
| Schema and code can drift apart | Schema generated from code—always in sync |
| Validation logic duplicated | Type system enforces validation |
| Easy to miss edge cases | Compiler catches type mismatches |
| String comparisons everywhere | Pattern matching on enums |
| Runtime type errors | Compile-time type safety |
| Verbose error handling | Automatic deserialization errors |

```rust
// ❌ Manual approach: error-prone, verbose
let format = params.get("format")
    .and_then(|v| v.as_str())
    .ok_or(ValidationError::missing_field("format"))?;
if !["json", "csv", "markdown"].contains(&format) {
    return Err(ValidationError::invalid_value("format", "..."));
}

// ✅ TypedTool approach: type-safe, concise
// format is already OutputFormat enum—invalid values rejected automatically
match args.format {
    OutputFormat::Json => generate_json_report(&data),
    OutputFormat::Csv => generate_csv_report(&data),
    OutputFormat::Markdown => generate_markdown_report(&data),
}
```

## Error Messages for AI Clients

Error messages should help the AI self-correct. Include:

1. **What was wrong**: Clear identification of the problem
2. **What was expected**: The correct format or value range
3. **What was received**: Echo back what the AI sent
4. **How to fix it**: Specific guidance

```rust
// Poor error message
Err(Error::new("Invalid input"))

// Good error message
Err(ValidationError {
    code: "INVALID_DATE_FORMAT",
    field: "date_range.start",
    message: "Date must be in ISO 8601 format (YYYY-MM-DD)",
    expected: "2024-11-01",
    received: "November 1st, 2024",
    suggestion: "Convert 'November 1st, 2024' to '2024-11-01'"
})
```

## Chapter Overview

This chapter covers three aspects of validation:

1. **Schema-Driven Validation**: Using JSON Schema effectively to prevent errors before they happen

2. **Output Schemas for Composition**: How declaring output structure helps AI clients chain tools together

3. **Type-Safe Tool Annotations**: Using Rust's type system and MCP annotations for additional safety

Good validation transforms errors from frustrating dead-ends into helpful guidance. When an AI client makes a mistake, your validation should teach it the right way.
