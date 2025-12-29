# Cohesive API Design

Cohesion in MCP server design means your tools, resources, and prompts form a unified, understandable whole—both for AI clients that must choose between them and for users who need predictable behavior.

## The Multi-Server Reality

Your MCP server operates in an environment you don't control. Consider what an AI client sees when a user has multiple servers connected:

```
Connected MCP Servers (typical business user setup):

1. google-drive-server (document storage)
   - create_document, update_document, delete_document,
   - list_documents, search_documents, share_document

2. asana-server (task management)
   - create_task, update_task, delete_task, list_tasks,
   - create_project, assign_task, set_due_date

3. salesforce-server (CRM)
   - query_accounts, update_opportunity, list_contacts, log_activity

4. your-server (you're building this)
   - ???

Total tools visible to AI: 20+ (and growing)
```

Your server's tools must be instantly distinguishable in this crowded environment.

## Principles of Cohesive Design

### 1. Domain Prefixing

Prefix tool names with your domain to avoid collisions:

```rust
// Collision risk: generic names
Tool::new("query")           // Collides with postgres-server
Tool::new("search")          // Collides with Google Drive search_documents
Tool::new("list")            // Collides with everything

// Cohesive: domain-specific names
Tool::new("sales_query")     // Clearly your sales system
Tool::new("sales_report")    // Consistent prefix
Tool::new("sales_forecast")  // AI understands these are related
```

The AI can now reason: "The user asked about sales, I'll use the `sales_*` tools."

### 2. Consistent Verb Patterns

Choose a verb convention and stick to it across all tools:

```rust
// Inconsistent verbs (confusing)
Tool::new("get_customer")       // "get"
Tool::new("fetch_orders")       // "fetch" - same meaning, different word
Tool::new("retrieve_products")  // "retrieve" - yet another synonym
Tool::new("load_inventory")     // "load" - and another

// Consistent verbs (cohesive)
Tool::new("get_customer")
Tool::new("get_orders")
Tool::new("get_products")
Tool::new("get_inventory")
```

Consistent patterns help the AI predict tool names and understand tool relationships.

### 3. Hierarchical Organization

Structure tools to reflect their relationships:

```rust
// Flat structure (hard to understand relationships)
vec![
    Tool::new("create_order"),
    Tool::new("add_item"),
    Tool::new("remove_item"),
    Tool::new("apply_discount"),
    Tool::new("calculate_total"),
    Tool::new("submit_order"),
    Tool::new("cancel_order"),
]

// Hierarchical structure (clear relationships)
// Order lifecycle tools
Tool::new("order_create")
    .description("Create a new order. Returns order_id for subsequent operations.")

Tool::new("order_modify")
    .description("Add items, remove items, or apply discounts to an existing order.")
    .input_schema(json!({
        "properties": {
            "order_id": { "type": "string" },
            "action": {
                "type": "string",
                "enum": ["add_item", "remove_item", "apply_discount"]
            }
        }
    }))

Tool::new("order_finalize")
    .description("Calculate totals and submit the order, or cancel it.")
    .input_schema(json!({
        "properties": {
            "order_id": { "type": "string" },
            "action": {
                "type": "string",
                "enum": ["submit", "cancel"]
            }
        }
    }))
```

Three tools instead of seven, with clear lifecycle stages.

## Designing for AI Understanding

### Description Templates

Use consistent description structures across all tools:

```rust
// Template: What it does | When to use it | What it returns

Tool::new("sales_query")
    .description(
        "Execute SQL queries against the sales database. \
        Use for retrieving sales records, revenue data, and transaction history. \
        Returns query results as JSON array of records."
    )

Tool::new("sales_report")
    .description(
        "Generate formatted sales reports for a date range. \
        Use when the user needs summaries, trends, or printable reports. \
        Returns report data with totals, averages, and visualizable metrics."
    )

Tool::new("sales_forecast")
    .description(
        "Predict future sales based on historical data. \
        Use when the user asks about projections, predictions, or planning. \
        Returns forecast data with confidence intervals."
    )
```

The AI can now distinguish:
- Raw data needs → `sales_query`
- Summaries/reports → `sales_report`
- Future predictions → `sales_forecast`

### Negative Descriptions

Sometimes it helps to say what a tool is *not* for:

```rust
Tool::new("sales_query")
    .description(
        "Execute read-only SQL queries against the sales database. \
        Use for retrieving sales records and transaction history. \
        \
        NOTE: This tool CANNOT modify data. For updates, use sales_admin. \
        NOTE: For reports and summaries, use sales_report instead (faster)."
    )
```

### Output Consistency

Tools in the same domain should return consistent structures:

```rust
// All sales tools return a consistent envelope
{
    "success": true,
    "data": { /* tool-specific data */ },
    "metadata": {
        "query_time_ms": 45,
        "source": "sales_db_replica",
        "cached": false
    }
}
```

This helps the AI chain tools together—it knows what to expect.

## Cohesion Across Tool-Resource-Prompt

True cohesion spans all three MCP primitives:

```rust
// TOOLS: Actions on the sales domain
Tool::new("sales_query")
Tool::new("sales_report")
Tool::new("sales_forecast")

// RESOURCES: Reference data for sales operations
Resource::new("sales://schema")
    .description("Sales database schema - tables, columns, relationships")
Resource::new("sales://regions")
    .description("List of sales regions with IDs and territories")
Resource::new("sales://products")
    .description("Product catalog with IDs, names, and categories")

// PROMPTS: Guided workflows combining tools and resources
Prompt::new("quarterly-sales-analysis")
    .description("Comprehensive quarterly sales analysis with trends and forecasts")
Prompt::new("sales-territory-review")
    .description("Review sales performance by territory with recommendations")
```

The AI sees a complete, cohesive sales domain:
- **Resources** provide context (what data exists)
- **Tools** provide actions (what can be done)
- **Prompts** provide workflows (how to accomplish complex tasks)

## Testing Cohesion

### The "50 Tools" Test

List all tools from your server plus common business servers (Google Drive, Asana, Salesforce). Can an AI easily distinguish yours?

```
google-drive: create_document, update_document, list_documents
asana: create_task, update_task, list_tasks
salesforce: query_accounts, update_opportunity, list_contacts
your-server: ???

If your tools are "query", "list", "get" - FAIL
If your tools are "sales_query", "sales_report", "sales_forecast" - PASS
```

### The "Explain It" Test

Describe your server to a colleague in one sentence. If you can't, your tools aren't cohesive.

```
FAIL: "It queries databases, generates reports, and also manages inventory
       and does some customer stuff"

PASS: "It provides sales analytics - querying historical data, generating
       reports, and forecasting future sales"
```

### The "New Tool" Test

When you add a new tool, does its name and description obviously fit with existing tools?

```
Existing: sales_query, sales_report, sales_forecast

Adding customer support?
FAIL: support_ticket, help_request  (different domain)
PASS: Create a new server for customer support

Adding sales alerts?
PASS: sales_alert_create, sales_alert_list (same domain, consistent naming)
```

## Advanced: Foundation and Domain Servers

As your organization scales MCP adoption, cohesion becomes even more critical. In [Part VIII: Server Composition](../part8-advanced/ch19-composition.md), we explore a powerful pattern: **Foundation Servers** wrapped by **Domain Servers**.

### The Pattern

Instead of building monolithic servers or having every team create their own database tools, you create a layered architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    Business Users                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────┐  ┌──────────────────┐                 │
│  │  Sales Manager   │  │  Finance Manager │  Domain Servers │
│  │  Domain Server   │  │  Domain Server   │  (department-   │
│  │                  │  │                  │   specific)     │
│  │ • pipeline_view  │  │ • budget_check   │                 │
│  │ • territory_perf │  │ • expense_report │                 │
│  │ • forecast_q4    │  │ • revenue_audit  │                 │
│  └────────┬─────────┘  └────────┬─────────┘                 │
│           │                     │                            │
│           └──────────┬──────────┘                            │
│                      │                                       │
│           ┌──────────▼──────────┐                            │
│           │   Foundation Server │  Foundation Server         │
│           │   (db-explorer)     │  (general-purpose)         │
│           │                     │                            │
│           │   • db_query        │                            │
│           │   • db_schema       │                            │
│           │   • db_export       │                            │
│           └─────────────────────┘                            │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Why This Matters for Cohesion

**Foundation Servers** are general-purpose, reusable across the organization:
- `db-explorer`: Generic database access
- `file-manager`: Document and file operations
- `api-gateway`: External API integrations

**Domain Servers** wrap foundations with business-specific cohesion:
- Focused on one department's workflows
- Pre-configured with relevant schemas and permissions
- Include prompts tailored to that department's tasks
- Hide complexity that's irrelevant to those users

```rust
// Sales Manager Domain Server
// Wraps db-explorer but exposes only sales-relevant operations

Tool::new("pipeline_view")
    .description("View sales pipeline with deal stages and probabilities")
    // Internally calls db_query with pre-built sales pipeline query

Tool::new("territory_performance")
    .description("Compare territory performance against targets")
    // Internally calls db_query + db_export for territory reports

Prompt::new("weekly-forecast")
    .description("Generate weekly sales forecast for your territories")
    // Guides the manager through a structured forecasting workflow
```

### Benefits

1. **User-Appropriate Cohesion**: Sales managers see sales tools, not raw SQL
2. **Controlled Access**: Domain servers enforce what each role can access
3. **Maintainability**: Update the foundation; all domain servers benefit
4. **Reduced Tool Sprawl**: Each user sees only 5-10 relevant tools, not 50

### When to Use This Pattern

- Multiple departments need different views of the same data
- You want to control what each role can access
- Business users shouldn't need to understand database schemas
- You're scaling from one team to organization-wide MCP adoption

We cover this pattern in depth in [Chapter 19: Server Composition](../part8-advanced/ch19-composition.md), including implementation details, authentication flows, and real-world examples.

## Summary

Cohesive design makes your MCP server:
- **Distinguishable**: AI easily identifies your tools among many servers
- **Predictable**: Users know what to expect from your domain
- **Maintainable**: New tools fit naturally into existing patterns

The key insight: design for the multi-server environment from the start. Your tools don't exist in isolation—they compete for the AI's attention alongside dozens of other tools.

Next, we'll examine the single responsibility principle—why each tool should do one thing well.
