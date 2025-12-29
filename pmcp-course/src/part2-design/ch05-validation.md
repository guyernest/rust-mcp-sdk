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

```rust
pub async fn handle_sales_query(params: Value) -> Result<Value> {
    // 1. Schema validation (type checking)
    let query = params.get("query")
        .and_then(|v| v.as_str())
        .ok_or(ValidationError::missing_field("query"))?;

    // 2. Format validation
    let date_range = if let Some(dr) = params.get("date_range") {
        Some(DateRange::parse(dr)?)  // Validates ISO 8601 format
    } else {
        None
    };

    // 3. Business validation
    if let Some(ref dr) = date_range {
        if dr.end > Utc::now().date_naive() {
            return Err(ValidationError::invalid_value(
                "date_range.end",
                "Cannot query future dates"
            ));
        }
    }

    // 4. Security validation
    if contains_dangerous_sql(query) {
        return Err(ValidationError::security(
            "Query contains disallowed SQL constructs"
        ));
    }

    // Execute query...
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
