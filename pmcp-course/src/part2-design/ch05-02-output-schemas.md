# Output Schemas for Composition

Input validation prevents errors. Output schemas enable composition. When AI clients know what your tool returns, they can chain operations together confidently.

## The Composition Challenge

Consider an AI trying to use two tools together:

```
User: "Get our top customers and analyze their recent orders"

AI reasoning:
1. Use sales_top_customers to get customer list
2. For each customer, use order_history to get orders
3. Analyze patterns across all orders

But wait:
- What does sales_top_customers return?
- Is there a customer_id field? Or is it id? Or customer?
- What format is the response in?
- How do I iterate over the results?
```

Without knowing the output structure, the AI must guess—or execute the first tool and inspect results before continuing.

## Declaring Output Schemas

MCP tools can declare their output structure, giving AI clients advance knowledge of what to expect:

```rust
Tool::new("sales_top_customers")
    .description("Get top customers by revenue for a time period")
    .input_schema(json!({
        "type": "object",
        "properties": {
            "period": { "type": "string", "enum": ["month", "quarter", "year"] },
            "limit": { "type": "integer", "default": 10 }
        }
    }))
    .output_schema(json!({
        "type": "object",
        "properties": {
            "customers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "customer_id": { "type": "string" },
                        "name": { "type": "string" },
                        "total_revenue": { "type": "number" },
                        "order_count": { "type": "integer" },
                        "last_order_date": { "type": "string", "format": "date" }
                    }
                }
            },
            "period": { "type": "string" },
            "generated_at": { "type": "string", "format": "date-time" }
        }
    }))
```

Now the AI knows:
- Results are in a `customers` array
- Each customer has `customer_id` (not `id` or `customer`)
- Revenue is a number, order_count is an integer
- Dates are in ISO 8601 format

## Consistent Response Envelopes

Design output schemas with consistent patterns across all tools:

### The Standard Envelope

```rust
// All sales tools return this structure
json!({
    "type": "object",
    "required": ["success", "data"],
    "properties": {
        "success": {
            "type": "boolean",
            "description": "Whether the operation succeeded"
        },
        "data": {
            "type": "object",
            "description": "Tool-specific response data"
        },
        "metadata": {
            "type": "object",
            "properties": {
                "execution_time_ms": { "type": "integer" },
                "source": { "type": "string" },
                "cached": { "type": "boolean" }
            }
        },
        "error": {
            "type": "object",
            "description": "Present only when success is false",
            "properties": {
                "code": { "type": "string" },
                "message": { "type": "string" },
                "details": { "type": "object" }
            }
        }
    }
})
```

With a consistent envelope, the AI learns one pattern for all your tools:

```rust
// AI can reliably process any tool output
if response.success {
    let data = response.data;
    // Process tool-specific data
} else {
    let error = response.error;
    // Handle error, maybe retry
}
```

### Implementation

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ToolResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub metadata: ResponseMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDetail>,
}

#[derive(Serialize)]
pub struct ResponseMetadata {
    pub execution_time_ms: u64,
    pub source: String,
    pub cached: bool,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T: Serialize> ToolResponse<T> {
    pub fn success(data: T, metadata: ResponseMetadata) -> Self {
        Self {
            success: true,
            data: Some(data),
            metadata,
            error: None,
        }
    }

    pub fn error(error: ErrorDetail, metadata: ResponseMetadata) -> Self {
        Self {
            success: false,
            data: None,
            metadata,
            error: Some(error),
        }
    }
}
```

## Designing for Chaining

Structure outputs to support common chaining patterns:

### IDs for Follow-up Operations

When a tool returns entities, include IDs that work with other tools:

```rust
// sales_top_customers returns customer_id
{
    "customers": [
        { "customer_id": "CUST-001", "name": "Acme Corp", ... },
        { "customer_id": "CUST-002", "name": "TechStart Inc", ... }
    ]
}

// order_history accepts customer_id
Tool::new("order_history")
    .input_schema(json!({
        "properties": {
            "customer_id": {
                "type": "string",
                "description": "Customer ID from sales_top_customers or customer_search"
            }
        }
    }))
```

The AI sees `customer_id` in both places and understands how to chain them.

### Pagination Cursors

For paginated results, return consistent cursor information:

```rust
json!({
    "type": "object",
    "properties": {
        "results": {
            "type": "array",
            "items": { /* result schema */ }
        },
        "pagination": {
            "type": "object",
            "properties": {
                "total_count": { "type": "integer" },
                "page_size": { "type": "integer" },
                "has_more": { "type": "boolean" },
                "next_cursor": {
                    "type": "string",
                    "description": "Pass to 'cursor' parameter to get next page"
                }
            }
        }
    }
})
```

The AI learns: if `has_more` is true, call again with `cursor: next_cursor`.

### Aggregation-Ready Data

When data might be aggregated, use consistent numeric fields:

```rust
// Each tool returns numbers in consistent units
json!({
    "properties": {
        "revenue": {
            "type": "number",
            "description": "Revenue in USD cents (divide by 100 for dollars)"
        },
        "quantity": {
            "type": "integer",
            "description": "Number of units"
        },
        "percentage": {
            "type": "number",
            "description": "Percentage as decimal (0.15 = 15%)"
        }
    }
})
```

## Output Schema Documentation

The schema itself is documentation. Make field purposes clear:

### Documenting Fields

```rust
json!({
    "type": "object",
    "properties": {
        "customer_id": {
            "type": "string",
            "description": "Unique identifier. Use with order_history, customer_details tools."
        },
        "segment": {
            "type": "string",
            "enum": ["enterprise", "mid-market", "smb"],
            "description": "Customer segment based on annual revenue"
        },
        "health_score": {
            "type": "integer",
            "minimum": 0,
            "maximum": 100,
            "description": "Customer health score. <50=at-risk, 50-80=stable, >80=healthy"
        },
        "last_contact": {
            "type": "string",
            "format": "date-time",
            "description": "Last customer interaction timestamp (UTC)"
        }
    }
})
```

### Documenting Relationships

```rust
// In tool description
Tool::new("sales_pipeline")
    .description(
        "Get sales pipeline data. \
        Returns opportunity_id values that can be used with: \
        - opportunity_details (full opportunity info) \
        - opportunity_contacts (related contacts) \
        - opportunity_activities (activity timeline)"
    )
    .output_schema(json!({
        "type": "object",
        "properties": {
            "opportunities": {
                "type": "array",
                "description": "List of opportunities. Each opportunity_id works with opportunity_* tools.",
                "items": {
                    "type": "object",
                    "properties": {
                        "opportunity_id": {
                            "type": "string",
                            "description": "Use with opportunity_details, opportunity_contacts, opportunity_activities"
                        }
                        // ...
                    }
                }
            }
        }
    }))
```

## Error Output Schemas

Define what errors look like so the AI can handle them:

```rust
// Error schema as part of output
json!({
    "type": "object",
    "oneOf": [
        {
            "type": "object",
            "required": ["success", "data"],
            "properties": {
                "success": { "const": true },
                "data": { /* success schema */ }
            }
        },
        {
            "type": "object",
            "required": ["success", "error"],
            "properties": {
                "success": { "const": false },
                "error": {
                    "type": "object",
                    "required": ["code", "message"],
                    "properties": {
                        "code": {
                            "type": "string",
                            "enum": [
                                "NOT_FOUND",
                                "INVALID_QUERY",
                                "RATE_LIMITED",
                                "PERMISSION_DENIED",
                                "TIMEOUT"
                            ]
                        },
                        "message": { "type": "string" },
                        "retry_after_seconds": {
                            "type": "integer",
                            "description": "Present for RATE_LIMITED errors"
                        }
                    }
                }
            }
        }
    ]
})
```

This tells the AI:
- Success responses have `success: true` and a `data` field
- Error responses have `success: false` and an `error` field
- Error codes are from a known set
- Rate limit errors include retry guidance

## Validation of Outputs

Just as you validate inputs, validate outputs before returning:

```rust
use serde::Serialize;
use schemars::JsonSchema;

#[derive(Serialize, JsonSchema)]
pub struct CustomerReport {
    pub customer_id: String,
    pub name: String,
    pub total_revenue: f64,
    pub order_count: u32,
}

pub async fn generate_report(params: ReportParams) -> Result<ToolResponse<CustomerReport>> {
    let report = build_report(&params).await?;

    // Validate output matches schema expectations
    if report.total_revenue < 0.0 {
        return Err(InternalError::new(
            "Generated report has negative revenue - data integrity issue"
        ));
    }

    if report.customer_id.is_empty() {
        return Err(InternalError::new(
            "Generated report missing customer_id"
        ));
    }

    Ok(ToolResponse::success(report, metadata))
}
```

## Schema Generation from Types

Use derive macros to generate schemas from Rust types:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SalesReport {
    /// Unique report identifier
    pub report_id: String,

    /// Report generation timestamp (UTC)
    pub generated_at: chrono::DateTime<chrono::Utc>,

    /// Sales data grouped by region
    pub by_region: Vec<RegionSales>,

    /// Summary statistics
    pub summary: SalesSummary,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct RegionSales {
    /// Region identifier (e.g., "NA", "EMEA", "APAC")
    pub region: String,

    /// Total revenue in USD cents
    pub revenue_cents: i64,

    /// Number of orders
    pub order_count: u32,

    /// Average order value in USD cents
    pub avg_order_value_cents: i64,
}

// Generate schema at compile time
fn sales_report_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(SalesReport)).unwrap()
}
```

## Summary

Output schemas enable composition by telling AI clients:

| What to Document | Why It Matters |
|------------------|----------------|
| **Field names and types** | AI constructs follow-up operations correctly |
| **ID relationships** | AI knows how to chain tools together |
| **Consistent envelopes** | AI learns one pattern for all your tools |
| **Error structures** | AI can handle failures gracefully |
| **Units and formats** | AI interprets values correctly |
| **Pagination patterns** | AI knows how to get more results |

Remember: output schemas are a contract. The AI trusts that your tool returns what you declare. Validate your outputs to maintain that trust.
