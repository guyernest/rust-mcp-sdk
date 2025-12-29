# Schema-Driven Validation

JSON Schema is your first line of defense—and your first opportunity to communicate with AI clients. A well-designed schema prevents errors before they happen and guides AI toward correct parameter construction.

## Schema as Documentation

When an AI client encounters your tool, it reads the schema to understand what parameters are valid. The schema serves multiple purposes:

```rust
Tool::new("sales_query")
    .description("Execute read-only SQL queries against the sales database")
    .input_schema(json!({
        "type": "object",
        "required": ["query"],
        "properties": {
            "query": {
                "type": "string",
                "description": "SQL SELECT statement to execute",
                "minLength": 1,
                "maxLength": 10000
            },
            "limit": {
                "type": "integer",
                "description": "Maximum rows to return (default: 100, max: 10000)",
                "minimum": 1,
                "maximum": 10000,
                "default": 100
            },
            "timeout_ms": {
                "type": "integer",
                "description": "Query timeout in milliseconds",
                "minimum": 100,
                "maximum": 30000,
                "default": 5000
            }
        }
    }))
```

This schema tells the AI:
- `query` is required, must be a non-empty string
- `limit` is optional with sensible bounds
- `timeout_ms` has reasonable defaults

## Essential Schema Patterns

### Required vs Optional Fields

Use `required` to distinguish mandatory from optional parameters:

```rust
json!({
    "type": "object",
    "required": ["customer_id"],  // Must provide customer_id
    "properties": {
        "customer_id": {
            "type": "string",
            "description": "Unique customer identifier (required)"
        },
        "include_history": {
            "type": "boolean",
            "description": "Include order history (optional, default: false)",
            "default": false
        }
    }
})
```

### Enum Constraints

When parameters have a fixed set of valid values, use enums:

```rust
json!({
    "type": "object",
    "properties": {
        "region": {
            "type": "string",
            "enum": ["north", "south", "east", "west"],
            "description": "Sales region to query"
        },
        "format": {
            "type": "string",
            "enum": ["json", "csv", "markdown"],
            "description": "Output format for results",
            "default": "json"
        }
    }
})
```

Enums help the AI choose correctly. Without an enum, the AI might try "JSON", "Json", or "application/json".

### Nested Objects

For complex parameters, use nested objects with their own schemas:

```rust
json!({
    "type": "object",
    "required": ["date_range"],
    "properties": {
        "date_range": {
            "type": "object",
            "description": "Date range for the query",
            "required": ["start", "end"],
            "properties": {
                "start": {
                    "type": "string",
                    "format": "date",
                    "description": "Start date (ISO 8601: YYYY-MM-DD)"
                },
                "end": {
                    "type": "string",
                    "format": "date",
                    "description": "End date (ISO 8601: YYYY-MM-DD)"
                }
            }
        }
    }
})
```

### Arrays with Item Schemas

When accepting lists, define what the list contains:

```rust
json!({
    "type": "object",
    "properties": {
        "product_ids": {
            "type": "array",
            "description": "List of product IDs to query",
            "items": {
                "type": "string",
                "pattern": "^PRD-[0-9]{6}$"
            },
            "minItems": 1,
            "maxItems": 100
        },
        "metrics": {
            "type": "array",
            "description": "Metrics to include in report",
            "items": {
                "type": "string",
                "enum": ["revenue", "units", "margin", "growth"]
            },
            "uniqueItems": true
        }
    }
})
```

## Format Specifications

JSON Schema supports format hints that help AI clients construct correct values:

| Format | Description | Example |
|--------|-------------|---------|
| `date` | ISO 8601 date | `2024-11-15` |
| `date-time` | ISO 8601 datetime | `2024-11-15T14:30:00Z` |
| `time` | ISO 8601 time | `14:30:00` |
| `email` | Email address | `user@example.com` |
| `uri` | URI/URL | `https://example.com/path` |
| `uuid` | UUID | `550e8400-e29b-41d4-a716-446655440000` |

```rust
json!({
    "type": "object",
    "properties": {
        "email": {
            "type": "string",
            "format": "email",
            "description": "Customer email address"
        },
        "created_after": {
            "type": "string",
            "format": "date-time",
            "description": "Filter to records created after this timestamp"
        },
        "callback_url": {
            "type": "string",
            "format": "uri",
            "description": "Webhook URL for async notifications"
        }
    }
})
```

## Pattern Validation

For custom formats, use regex patterns:

```rust
json!({
    "type": "object",
    "properties": {
        "order_id": {
            "type": "string",
            "pattern": "^ORD-[0-9]{4}-[A-Z]{2}-[0-9]{6}$",
            "description": "Order ID (format: ORD-YYYY-RR-NNNNNN, e.g., ORD-2024-NA-000123)"
        },
        "phone": {
            "type": "string",
            "pattern": "^\\+[1-9]\\d{1,14}$",
            "description": "Phone number in E.164 format (e.g., +14155551234)"
        }
    }
})
```

**Important**: Include an example in the description. The AI may not perfectly interpret the regex, but will use the example.

## Implementing Validation in Rust

Schema validation happens at two levels: the MCP client may validate before sending, and your server should validate on receipt.

### Basic Validation with Serde

Use serde to parse and validate input:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SalesQueryParams {
    pub query: String,

    #[serde(default = "default_limit")]
    pub limit: u32,

    #[serde(default = "default_timeout")]
    pub timeout_ms: u32,
}

fn default_limit() -> u32 { 100 }
fn default_timeout() -> u32 { 5000 }

pub async fn handle_sales_query(params: Value) -> Result<Value> {
    // Parse with serde - handles type validation
    let params: SalesQueryParams = serde_json::from_value(params)
        .map_err(|e| ValidationError::parse_error(e))?;

    // Additional validation beyond what schema can express
    if params.limit > 10000 {
        return Err(ValidationError::invalid_value(
            "limit",
            "Maximum limit is 10000",
            params.limit.to_string()
        ));
    }

    // Proceed with validated params
    execute_query(&params).await
}
```

### Validation with Detailed Errors

For AI-friendly error messages, create a validation helper:

```rust
pub struct ValidationError {
    pub code: String,
    pub field: String,
    pub message: String,
    pub expected: Option<String>,
    pub received: Option<String>,
}

impl ValidationError {
    pub fn missing_field(field: &str) -> Self {
        Self {
            code: "MISSING_REQUIRED_FIELD".into(),
            field: field.into(),
            message: format!("Required field '{}' is missing", field),
            expected: Some(format!("A value for '{}'", field)),
            received: None,
        }
    }

    pub fn invalid_type(field: &str, expected: &str, received: &str) -> Self {
        Self {
            code: "INVALID_TYPE".into(),
            field: field.into(),
            message: format!("Field '{}' has wrong type", field),
            expected: Some(expected.into()),
            received: Some(received.into()),
        }
    }

    pub fn invalid_value(field: &str, message: &str, received: String) -> Self {
        Self {
            code: "INVALID_VALUE".into(),
            field: field.into(),
            message: message.into(),
            expected: None,
            received: Some(received),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "error": {
                "code": self.code,
                "field": self.field,
                "message": self.message,
                "expected": self.expected,
                "received": self.received
            }
        })
    }
}
```

### Comprehensive Validation Function

```rust
pub fn validate_sales_query(params: &Value) -> Result<SalesQueryParams, ValidationError> {
    // 1. Check required fields
    let query = params.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::missing_field("query"))?;

    if query.is_empty() {
        return Err(ValidationError::invalid_value(
            "query",
            "Query cannot be empty",
            "".into()
        ));
    }

    // 2. Validate optional fields with defaults
    let limit = match params.get("limit") {
        Some(Value::Number(n)) => {
            n.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| ValidationError::invalid_type(
                    "limit",
                    "positive integer",
                    &n.to_string()
                ))?
        }
        Some(v) => {
            return Err(ValidationError::invalid_type(
                "limit",
                "integer",
                &format!("{:?}", v)
            ));
        }
        None => 100,  // default
    };

    if limit > 10000 {
        return Err(ValidationError::invalid_value(
            "limit",
            "Maximum allowed value is 10000",
            limit.to_string()
        ));
    }

    // 3. Return validated struct
    Ok(SalesQueryParams {
        query: query.to_string(),
        limit,
        timeout_ms: extract_timeout(params)?,
    })
}
```

## Common Validation Mistakes

### Don't: Silent Coercion

```rust
// BAD: Silently converts or ignores invalid values
let limit = params.get("limit")
    .and_then(|v| v.as_u64())
    .unwrap_or(100);  // AI never learns its mistake
```

### Do: Explicit Errors

```rust
// GOOD: Tell the AI what went wrong
let limit = match params.get("limit") {
    Some(Value::Number(n)) if n.as_u64().is_some() => {
        n.as_u64().unwrap() as u32
    }
    Some(v) => {
        return Err(ValidationError::invalid_type(
            "limit",
            "positive integer",
            &format!("{}", v)
        ));
    }
    None => 100,
};
```

### Don't: Vague Error Messages

```rust
// BAD: AI can't learn from this
Err(Error::new("Invalid input"))
```

### Do: Specific, Actionable Errors

```rust
// GOOD: AI knows exactly what to fix
Err(ValidationError {
    code: "INVALID_DATE_FORMAT".into(),
    field: "date_range.start".into(),
    message: "Date must be in ISO 8601 format".into(),
    expected: Some("2024-11-15".into()),
    received: Some("November 15, 2024".into()),
})
```

## How AI Clients Use Error Messages

When your tool returns an error, the AI client sees it as part of the tool's output. This creates a feedback loop that enables self-correction:

```
┌─────────────────────────────────────────────────────────────┐
│ AI Client Reasoning After Error                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Tool call failed with:                                      │
│  {                                                           │
│    "error": {                                                │
│      "code": "INVALID_DATE_FORMAT",                         │
│      "field": "date_range.start",                           │
│      "expected": "2024-11-15",                              │
│      "received": "November 15, 2024"                        │
│    }                                                         │
│  }                                                           │
│                                                              │
│  AI reasoning:                                               │
│  - The date format was wrong                                 │
│  - I sent "November 15, 2024"                               │
│  - It expects "2024-11-15" (ISO 8601)                       │
│  - I'll retry with the correct format                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Retry with Corrected Parameters

Clear error messages enable the AI to immediately retry with fixed values:

```
Attempt 1: sales_query(date_range: {start: "November 15, 2024", ...})
           → Error: INVALID_DATE_FORMAT

Attempt 2: sales_query(date_range: {start: "2024-11-15", ...})
           → Success!
```

The AI learned from the error and self-corrected without user intervention.

### Try a Different Approach

Sometimes an error indicates the AI should try a completely different strategy:

```
Attempt 1: customer_lookup(email: "john@...")
           → Error: CUSTOMER_NOT_FOUND

AI reasoning:
- Customer doesn't exist with this email
- Maybe I should search by name instead
- Or ask the user for more information

Attempt 2: customer_search(name: "John Smith")
           → Success: Found 3 matching customers
```

### Error Codes Enable Programmatic Decisions

Structured error codes let AI clients make intelligent decisions:

```rust
// Your error response
{
    "error": {
        "code": "RATE_LIMITED",
        "message": "Too many requests",
        "retry_after_seconds": 30
    }
}

// AI can reason:
// - RATE_LIMITED means I should wait and retry
// - NOT_FOUND means I should try a different query
// - PERMISSION_DENIED means I should inform the user
// - INVALID_FORMAT means I should fix my parameters
```

### The Feedback Loop

This creates a powerful feedback loop:

1. **AI attempts** a tool call based on schema understanding
2. **Tool validates** and returns structured error if invalid
3. **AI reads** the error in the tool output
4. **AI adjusts** its approach based on error details
5. **AI retries** with corrected parameters or different strategy

Without clear error messages, this loop breaks down. The AI either gives up or keeps making the same mistake.

## Schema Validation Libraries

For more sophisticated validation, consider schema validation libraries:

```rust
use jsonschema::{JSONSchema, Draft};

pub struct ValidatedTool {
    schema: JSONSchema,
}

impl ValidatedTool {
    pub fn new(schema: Value) -> Self {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema)
            .expect("Invalid schema");

        Self { schema: compiled }
    }

    pub fn validate(&self, params: &Value) -> Result<(), Vec<ValidationError>> {
        let result = self.schema.validate(params);

        if let Err(errors) = result {
            let validation_errors: Vec<ValidationError> = errors
                .map(|e| ValidationError {
                    code: "SCHEMA_VIOLATION".into(),
                    field: e.instance_path.to_string(),
                    message: e.to_string(),
                    expected: None,
                    received: Some(format!("{}", e.instance)),
                })
                .collect();

            return Err(validation_errors);
        }

        Ok(())
    }
}
```

## Summary

Schema-driven validation is about communication:

| Aspect | Purpose |
|--------|---------|
| **Required fields** | Tell AI what it must provide |
| **Types and formats** | Guide AI to correct data shapes |
| **Enums** | Constrain to valid choices |
| **Patterns with examples** | Show exact expected format |
| **Clear error messages** | Help AI self-correct |

Remember: the schema isn't just for validation—it's the primary documentation the AI uses to construct parameters. Make it clear, specific, and helpful.
