::: exercise
id: ch04-01-tool-design-review
difficulty: intermediate
time: 30 minutes
:::

A startup has asked you to review their MCP server design before they deploy it
to production. Their server started as a direct conversion of their REST API,
and they're concerned about usability.

Your task is to identify the design problems and propose a refactored design
that follows the principles from this chapter.

::: objectives
thinking:
  - Recognize anti-patterns in tool design (API-to-MCP trap, swiss army knife)
  - Apply cohesive naming with domain prefixes
  - Understand single responsibility for tools
  - Design for the multi-server environment
doing:
  - Analyze a problematic tool set and identify issues
  - Refactor tool names for cohesion and discoverability
  - Split multi-purpose tools into focused single-purpose tools
  - Write clear descriptions that help AI distinguish tools
:::

::: discussion
- What would happen if a user connects this server alongside GitHub and filesystem servers?
- How would an AI decide which tool to use for "show me recent activity"?
- What's the difference between a good tool set for humans vs. AI?
:::

::: starter file="review.md"
```markdown
# MCP Server Design Review

## Current Tool Set

The "business-tools" MCP server exposes these tools:

### Tool 1: `query`
- Description: "Query data"
- Parameters: { type: string, table: string, filters: object }
- Handles: SELECT from any table

### Tool 2: `modify`
- Description: "Modify data"
- Parameters: { type: string, operation: string, table: string, data: object }
- Operations: insert, update, delete
- Handles: All write operations for any table

### Tool 3: `get`
- Description: "Get something"
- Parameters: { what: string, id: string }
- Returns: customer, order, product, or user by ID

### Tool 4: `list`
- Description: "List things"
- Parameters: { what: string, page: number }
- Returns: paginated list of customers, orders, products, or users

### Tool 5: `report`
- Description: "Generate report"
- Parameters: { type: string, format: string, options: object }
- Types: sales, inventory, customers, financial

### Tool 6: `action`
- Description: "Perform action"
- Parameters: { action: string, target: string, data: object }
- Actions: send_email, create_ticket, archive, export

---

## Your Review Tasks

### Task 1: Identify Problems
For each tool, identify what anti-patterns it exhibits:
- [ ] Generic name (collision risk)
- [ ] Swiss army knife (too many responsibilities)
- [ ] Vague description (AI can't understand purpose)
- [ ] Unclear parameters (AI will guess wrong)

### Task 2: Propose Tool Groupings
What domain-specific tool groups would you create?
1. Customer domain: ___
2. Order domain: ___
3. Reporting domain: ___
4. Admin domain: ___

### Task 3: Refactor One Tool
Take the `report` tool and refactor it into focused tools:

Original:
- report(type: "sales" | "inventory" | "customers" | "financial", ...)

Refactored:
- ???
- ???
- ???

### Task 4: Write Better Descriptions
Improve this description following the template:
"What it does | When to use it | What it returns"

Original: "Query data"
Improved: ___

### Task 5: The "50 Tools Test"
Imagine these tools alongside:
- filesystem: read_file, write_file, list_directory, search_files
- github: create_issue, list_issues, create_pull_request
- postgres: query, list_tables, describe_table

Which of the business-tools would collide or be confused?
```
:::

::: hint level=1 title="Identifying collision risks"
Look for generic names that other servers might also use:
- `query` - postgres-server also has `query`
- `list` - many servers have list operations
- `get` - very generic, could mean anything
- `action` - what kind of action?

Ask: "If I saw just this tool name, would I know which server it came from?"
:::

::: hint level=2 title="Domain groupings"
Consider organizing by business domain, not by operation type:

**Customer domain:**
- customer_get, customer_list, customer_update

**Order domain:**
- order_get, order_list, order_create, order_cancel

**Reporting domain:**
- report_sales, report_inventory, report_customers

**Admin domain:**
- admin_send_email, admin_create_ticket, admin_export
:::

::: hint level=3 title="Refactoring the report tool"
The report tool does 4 different things. Split by report type:

```
report_sales
  - Description: "Generate sales report with revenue, units, and trends.
    Use when user asks about sales performance, revenue, or sales trends.
    Returns report data with totals, comparisons, and visualizable metrics."
  - Parameters: { date_range, group_by, include_forecast }

report_inventory
  - Description: "Generate inventory status report with stock levels and alerts.
    Use when user asks about stock, inventory, or supply levels.
    Returns current stock, reorder alerts, and turnover metrics."
  - Parameters: { warehouse, category, include_projections }

report_customers
  - Description: "Generate customer analytics report with segments and health.
    Use when user asks about customer behavior, churn, or segments.
    Returns segment breakdown, health scores, and trend analysis."
  - Parameters: { segment, time_period, include_cohort_analysis }
```
:::

::: solution
```markdown
# MCP Server Design Review - Solution

## Problem Analysis

### Tool 1: `query`
**Problems:**
- ❌ Generic name - collides with postgres-server's `query`
- ❌ Vague description - "Query data" tells AI nothing
- ❌ Swiss army knife - queries any table with dynamic type

### Tool 2: `modify`
**Problems:**
- ❌ Swiss army knife - insert, update, AND delete in one tool
- ❌ Dangerous - no separation between safe and destructive operations
- ❌ Vague description and parameters

### Tool 3: `get`
**Problems:**
- ❌ Generic name - `get` is used everywhere
- ❌ Swiss army knife - gets customers, orders, products, or users
- ❌ Description "Get something" is useless

### Tool 4: `list`
**Problems:**
- ❌ Generic name - collides with many servers
- ❌ Swiss army knife - lists any entity type
- ❌ AI must guess what "things" to list

### Tool 5: `report`
**Problems:**
- ❌ Swiss army knife - 4 different report types
- ❌ AI must know all report types exist
- ❌ Different reports need different parameters

### Tool 6: `action`
**Problems:**
- ❌ Extremely generic - "Perform action" on what?
- ❌ Mixes unrelated operations (email, tickets, archive, export)
- ❌ AI can't discover what actions are available

---

## Refactored Design

### Customer Domain
```
customer_get
  Description: "Get customer details by ID.
    Use when user asks about a specific customer.
    Returns profile, contact info, and account status."

customer_list
  Description: "List customers with optional filters.
    Use when user asks to see customers or search for customers.
    Returns paginated customer list with summary info."

customer_update
  Description: "Update customer information.
    Use when user explicitly requests customer changes.
    Returns updated customer record."
```

### Order Domain
```
order_get
  Description: "Get order details by ID.
    Use for order lookups and status checks.
    Returns order with items, status, and tracking."

order_list
  Description: "List orders with filters.
    Use for order history and order searches.
    Returns paginated orders with summary."

order_create
  Description: "Create a new order.
    Use when user wants to place an order.
    Returns created order with ID."
```

### Reporting Domain
```
report_sales
  Description: "Generate sales performance report.
    Use for revenue analysis, sales trends, and performance reviews.
    Returns totals, comparisons, and trend data."

report_inventory
  Description: "Generate inventory status report.
    Use for stock levels, reorder alerts, and supply planning.
    Returns stock levels and projections."

report_customer_analytics
  Description: "Generate customer analytics report.
    Use for churn analysis, segmentation, and customer health.
    Returns segment data and health metrics."
```

### Admin Domain
```
admin_send_email
  Description: "Send email to customer or internal recipient.
    Use when user explicitly requests sending an email.
    Returns send confirmation and tracking ID."

admin_export_data
  Description: "Export data to file format.
    Use when user needs data download or file export.
    Returns file path or download URL."
```

---

## Description Improvement

Original: "Query data"

Improved: "Execute read-only queries against the customer database.
Use for retrieving customer records, order history, and account details.
Returns query results as JSON array with pagination metadata."

---

## "50 Tools Test" Results

**Collisions identified:**
- `query` → Collides with postgres-server `query`
- `list` → Ambiguous with filesystem `list_directory`
- `get` → Too generic, could be anything

**After refactoring:**
- `customer_get`, `order_list`, `report_sales` → Instantly identifiable
- No collisions with filesystem, github, or postgres servers
- AI can distinguish by domain prefix
```
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    // These are conceptual tests for the exercise

    #[test]
    fn tool_names_have_domain_prefix() {
        let tool_names = vec![
            "customer_get",
            "customer_list",
            "order_create",
            "report_sales",
        ];

        for name in tool_names {
            assert!(
                name.contains("_"),
                "Tool {} should have domain prefix",
                name
            );
        }
    }

    #[test]
    fn descriptions_follow_template() {
        let description = "Execute read-only queries against the customer database. \
            Use for retrieving customer records. \
            Returns query results as JSON array.";

        assert!(description.contains("Use for"),
            "Description should explain when to use");
        assert!(description.contains("Returns"),
            "Description should explain what it returns");
    }
}
```
:::

::: reflection
- How would you handle a case where a tool legitimately needs to do multiple things?
- What's the trade-off between fewer multi-purpose tools and many focused tools?
- How might you document the relationships between related tools?
- Should you ever break the domain prefix convention? When?
:::
