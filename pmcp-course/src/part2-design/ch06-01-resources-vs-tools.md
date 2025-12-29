# When to Use Resources vs Tools

Resources and tools both provide data to AI clients, but they serve fundamentally different purposes. Understanding when to use each leads to cleaner designs and better AI behavior.

## The Key Distinction

| Aspect | Resources | Tools |
|--------|-----------|-------|
| **Purpose** | Provide stable data | Perform actions |
| **Identity** | Addressable by URI | Invoked by name |
| **Side effects** | None | May have side effects |
| **Caching** | Often cached by clients | Not cached |
| **AI perception** | Context/reference data | Operations to perform |

Think of it this way:
- **Resources** are nouns: "the customer schema", "the configuration"
- **Tools** are verbs: "query the database", "update the record"

## Decision Framework

Use this flowchart to decide:

```
┌─────────────────────────────────────────────────────────────┐
│ Does the operation have side effects?                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   YES ──► Use a TOOL                                        │
│           - Database modifications                           │
│           - External API calls that mutate                   │
│           - Sending notifications                            │
│           - Creating files                                   │
│                                                              │
│   NO ──► Does the data have a stable identity?              │
│          │                                                   │
│          ├─ YES ──► Use a RESOURCE                          │
│          │          - Schema definitions                     │
│          │          - Configuration                          │
│          │          - Reference data                         │
│          │          - Static documentation                   │
│          │                                                   │
│          └─ NO ──► Does it require computation?             │
│                    │                                         │
│                    ├─ YES ──► Use a TOOL                    │
│                    │          - Complex queries              │
│                    │          - Aggregations                 │
│                    │          - Reports                      │
│                    │                                         │
│                    └─ NO ──► Use a RESOURCE                 │
│                               - Simple lookups               │
│                               - Cached data                  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Resources: Best Use Cases

### 1. Schema and Structure Information

Schemas rarely change and are essential context:

```rust
// Database schema - AI reads to understand what queries are valid
Resource::new("db://schema/customers")
    .name("Customers Table Schema")
    .description("Column names, types, and relationships for customers table")
    .mime_type("application/json")

// API schema - AI reads to construct valid requests
Resource::new("api://openapi/v1")
    .name("API Specification")
    .description("OpenAPI specification for the REST API")
    .mime_type("application/json")
```

### 2. Configuration and Settings

Current configuration that guides tool usage:

```rust
// Feature flags - AI reads to know what's enabled
Resource::new("config://features")
    .name("Feature Flags")
    .description("Currently enabled features and experiments")

// Limits and quotas - AI reads to stay within bounds
Resource::new("config://limits")
    .name("Service Limits")
    .description("Rate limits, quotas, and maximum values")
```

### 3. Reference Data

Static or slowly-changing reference data:

```rust
// Region codes - AI reads when constructing queries
Resource::new("reference://regions")
    .name("Sales Regions")
    .description("Region codes, names, and territories")

// Product catalog - AI reads for lookups
Resource::new("reference://products")
    .name("Product Catalog")
    .description("Product IDs, names, categories, and attributes")
```

### 4. Documentation and Help

In-context documentation:

```rust
// Query syntax help
Resource::new("docs://sql-guide")
    .name("SQL Query Guide")
    .description("Supported SQL syntax with examples")

// Best practices
Resource::new("docs://best-practices")
    .name("API Best Practices")
    .description("Recommended patterns for using this API")
```

## Tools: Best Use Cases

### 1. Data Queries with Parameters

Queries that need runtime input:

```rust
// Query tool - parameters determine what's returned
Tool::new("sales_query")
    .description("Query sales data with filters")
    .input_schema(json!({
        "properties": {
            "date_range": { ... },
            "region": { ... },
            "product_category": { ... }
        }
    }))
```

### 2. Write Operations

Any operation that modifies state:

```rust
// Create operations
Tool::new("order_create")
    .description("Create a new order")

// Update operations
Tool::new("customer_update")
    .description("Update customer information")

// Delete operations
Tool::new("record_delete")
    .description("Delete a record")
```

### 3. External API Calls

Interactions with external services:

```rust
// Third-party integrations
Tool::new("send_email")
    .description("Send email via SendGrid")

// Payment processing
Tool::new("process_payment")
    .description("Process payment via Stripe")
```

### 4. Computed Results

Operations requiring significant computation:

```rust
// Aggregation
Tool::new("sales_report")
    .description("Generate sales report with totals and averages")

// Analysis
Tool::new("trend_analysis")
    .description("Analyze trends in historical data")
```

## Common Mistakes

### Mistake 1: Read Operations as Tools

```rust
// WRONG: This is just reading data
Tool::new("get_schema")
    .description("Get the database schema")

// RIGHT: Stable data should be a resource
Resource::new("db://schema")
    .description("Database schema")
```

### Mistake 2: Dynamic Data as Resources

```rust
// WRONG: This data changes based on parameters
Resource::new("sales://today")
    .description("Today's sales data")
// What if user needs yesterday's data?

// RIGHT: Parameterized queries should be tools
Tool::new("sales_query")
    .description("Query sales data for a date range")
    .input_schema(json!({
        "properties": {
            "date": { "type": "string", "format": "date" }
        }
    }))
```

### Mistake 3: Actions as Resources

```rust
// WRONG: Has side effects
Resource::new("notifications://send")
    .description("Send a notification")

// RIGHT: Side effects require tools
Tool::new("send_notification")
    .description("Send a notification to a user")
```

## Hybrid Patterns

Some scenarios benefit from both resources and tools:

### Resource for Context, Tool for Action

```rust
// Resource: schema for understanding
Resource::new("db://schema/orders")
    .description("Order table structure")

// Tool: query for action
Tool::new("order_query")
    .description("Query orders. See db://schema/orders for available columns.")
```

The AI reads the resource to understand the schema, then uses the tool to query.

### Resource Templates for Entities

```rust
// Template resource for specific entities
Resource::new("customers://{customer_id}")
    .name("Customer Details")
    .description("Read-only view of a specific customer")

// Tool for modifications
Tool::new("customer_update")
    .description("Update customer fields")
```

Reading customer details is a resource; modifying them is a tool.

### Cached Resources for Performance

```rust
// Expensive computation cached as resource
Resource::new("analytics://daily-summary")
    .name("Daily Summary")
    .description("Pre-computed daily analytics (updated hourly)")

// Real-time query as tool
Tool::new("analytics_query")
    .description("Real-time analytics query (slower, but up-to-date)")
```

## AI Behavior Differences

Resources and tools trigger different AI behaviors:

### Resources
- AI may read proactively to gather context
- Clients often cache resource contents
- AI doesn't count resource reads as "actions"
- Multiple reads don't concern the AI

### Tools
- AI calls tools deliberately to accomplish goals
- Each call is an "action" the AI considers
- AI may hesitate to call tools repeatedly
- Tool calls may require user confirmation

Design with these behaviors in mind:
- Put context-setting data in resources (AI reads freely)
- Put consequential operations in tools (AI considers carefully)

## Summary

| Use Resources For | Use Tools For |
|-------------------|---------------|
| Schemas and structure | Parameterized queries |
| Configuration | Write operations |
| Reference data | External integrations |
| Documentation | Computed results |
| Stable, addressable data | Actions with side effects |
| Context AI reads proactively | Operations AI performs deliberately |

The rule of thumb: if you'd bookmark it, it's a resource. If you'd submit a form, it's a tool.
