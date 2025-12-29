# Prompts as Workflow Templates

Prompts are your most powerful design tool for creating reliable, user-controlled workflows. While tools give the AI capabilities and resources provide context, prompts let users explicitly select how the AI should approach a task.

## Why Prompts Matter

Consider the difference:

**Without prompts:**
```
User: "Analyze our sales data"

AI (internally): I see 12 tools from 3 servers...
- sales_query (from sales-server)
- query (from postgres-server)
- read_file (from filesystem-server)
- search (from filesystem-server)
...

Which should I use? In what order? What analysis approach?
Let me just start querying and see what happens...
```

**With a prompt:**
```
User: /sales-analysis

Prompt template activates:
"I'll perform a comprehensive sales analysis:
1. First, read the schema to understand available data
2. Query key metrics: revenue, units, customers
3. Break down by region and time period
4. Compare against previous periods
5. Identify trends and anomalies

Starting with step 1..."
```

The prompt transforms an ambiguous request into a structured workflow.

## Prompt Design Principles

### 1. Be Explicit About Steps

The AI follows instructions better when steps are clearly numbered:

```rust
Prompt::new("database-audit")
    .description("Comprehensive database security audit")
    .messages(vec![
        PromptMessage::user(
            "Perform a security audit of the database:\
            \n\n**Step 1: Schema Analysis**\
            \n- Read db://schema to understand table structure\
            \n- Identify tables containing PII or sensitive data\
            \n\n**Step 2: Access Review**\
            \n- List all users with write permissions\
            \n- Flag any overly broad permission grants\
            \n\n**Step 3: Data Exposure Check**\
            \n- Check for unencrypted sensitive columns\
            \n- Verify no credentials stored in plain text\
            \n\n**Step 4: Report**\
            \n- Summarize findings with severity ratings\
            \n- Provide specific remediation recommendations\
            \n\nBegin with Step 1."
        )
    ])
```

### 2. Reference Specific Resources and Tools

Don't leave the AI guessing which tools to use:

```rust
Prompt::new("customer-360-view")
    .messages(vec![
        PromptMessage::user(
            "Create a 360-degree view of customer {{customer_id}}:\
            \n\n1. **Profile**: Read resource `customers://{{customer_id}}/profile`\
            \n2. **Orders**: Use `sales_query` to get order history\
            \n3. **Support**: Use `tickets_query` to get support interactions\
            \n4. **Payments**: Use `billing_query` to get payment history\
            \n\nSynthesize into a comprehensive customer summary."
        )
    ])
```

### 3. Define Output Format

Specify how results should be presented:

```rust
Prompt::new("weekly-metrics-report")
    .messages(vec![
        PromptMessage::user(
            "Generate the weekly metrics report:\
            \n\n## Data to Gather\
            \n- Revenue by region (use sales_aggregate)\
            \n- New customers (use customers_query)\
            \n- Support tickets (use tickets_summary)\
            \n\n## Output Format\
            \n```\
            \n# Weekly Metrics: {{week_start}} - {{week_end}}\
            \n\n## Revenue\
            \n| Region | This Week | Last Week | Change |\
            \n|--------|-----------|-----------|--------|\
            \n| ...    | ...       | ...       | ...    |\
            \n\n## Customer Acquisition\
            \n- New customers: X\
            \n- Churn: X\
            \n- Net growth: X\
            \n\n## Support Health\
            \n- Open tickets: X\
            \n- Avg response time: X\
            \n- CSAT: X%\
            \n\n## Key Insights\
            \n1. [Insight 1]\
            \n2. [Insight 2]\
            \n```"
        )
    ])
```

### 4. Include Guard Rails

Build safety checks into the workflow:

```rust
Prompt::new("data-modification")
    .description("Safely modify production data with review steps")
    .messages(vec![
        PromptMessage::user(
            "Help me modify data in {{table}}:\
            \n\n**Safety Protocol:**\
            \n1. First, show me the current state of affected records\
            \n2. Explain exactly what changes will be made\
            \n3. Ask for my explicit confirmation before proceeding\
            \n4. After modification, show the before/after comparison\
            \n\n**Constraints:**\
            \n- Maximum 100 records per operation\
            \n- No DELETE operations without WHERE clause\
            \n- All changes must be logged\
            \n\nWhat modification do you need?"
        )
    ])
```

## Advanced Prompt Patterns

### Multi-Turn Workflows

Prompts can define conversation structure:

```rust
Prompt::new("data-exploration")
    .description("Interactive data exploration session")
    .messages(vec![
        PromptMessage::user(
            "Start an interactive data exploration session:\
            \n\n**Initial Setup:**\
            \n1. Read available schemas\
            \n2. List tables and their row counts\
            \n3. Present a summary of available data\
            \n\n**Then wait for my questions. For each question:**\
            \n- If I ask about data: query and visualize\
            \n- If I ask about relationships: show joins and keys\
            \n- If I ask for export: use safe_export with confirmation\
            \n\n**Session rules:**\
            \n- Keep queries under 10,000 rows\
            \n- Warn before expensive operations\
            \n- Maintain context across questions\
            \n\nBegin setup."
        )
    ])
```

### Conditional Logic

Use template variables for dynamic behavior:

```rust
Prompt::new("anomaly-investigation")
    .arguments(vec![
        PromptArgument::new("severity")
            .description("Alert severity: low, medium, high, critical"),
        PromptArgument::new("metric")
            .description("The metric that triggered the alert"),
    ])
    .messages(vec![
        PromptMessage::user(
            "Investigate the {{severity}} severity anomaly in {{metric}}:\
            \n\n{{#if severity == 'critical'}}\
            \n**CRITICAL ALERT PROTOCOL:**\
            \n1. Immediately gather last 24 hours of data\
            \n2. Compare against last 7 days baseline\
            \n3. Identify correlated metrics\
            \n4. Check for system events at anomaly time\
            \n5. Prepare incident summary for escalation\
            \n{{else if severity == 'high'}}\
            \n**HIGH ALERT INVESTIGATION:**\
            \n1. Gather last 48 hours of data\
            \n2. Identify pattern or one-time spike\
            \n3. Check for known causes\
            \n4. Recommend monitoring or action\
            \n{{else}}\
            \n**STANDARD INVESTIGATION:**\
            \n1. Review metric trend for past week\
            \n2. Note if this is recurring\
            \n3. Log finding for pattern analysis\
            \n{{/if}}"
        )
    ])
```

### Chained Prompts

Design prompts that build on each other:

```rust
// First prompt: Discovery
Prompt::new("discover-opportunities")
    .description("Find potential opportunities in sales data")
    .messages(vec![
        PromptMessage::user(
            "Analyze sales data to identify opportunities:\
            \n\n1. Find underperforming products in growing categories\
            \n2. Identify customers with declining purchase frequency\
            \n3. Spot regions with untapped potential\
            \n\nList findings with IDs for follow-up analysis.\
            \nUser can then run /deep-dive on any finding."
        )
    ])

// Second prompt: Deep dive
Prompt::new("deep-dive")
    .arguments(vec![
        PromptArgument::new("finding_id")
            .description("ID from discover-opportunities output"),
    ])
    .description("Deep dive into a specific opportunity")
    .messages(vec![
        PromptMessage::user(
            "Perform detailed analysis on finding {{finding_id}}:\
            \n\n1. Gather all related data\
            \n2. Analyze root causes\
            \n3. Model potential impact of intervention\
            \n4. Provide specific, actionable recommendations\
            \n5. Estimate effort and expected return"
        )
    ])
```

## Client-Specific Considerations

### Claude Desktop

Claude Desktop shows prompts as slash commands in the input field:

```
/ [shows autocomplete list]
/quarterly-analysis
/customer-health-check
/data-exploration
```

Design prompts with short, memorable names:
- `/analyze-sales` not `/perform-comprehensive-sales-analysis-with-trend`
- `/health-check` not `/customer-health-check-and-churn-prediction`

### VS Code / Cursor

IDEs often show prompts in command palette:

```
> MCP: quarterly-analysis
> MCP: customer-health-check
> MCP: data-exploration
```

Include descriptions that explain the workflow:
```rust
Prompt::new("refactor-sql")
    .description("Safely refactor SQL queries with testing") // Shows in palette
```

### ChatGPT

ChatGPT may show prompts as conversation starters or actions:

```
┌─────────────────────────────────────────┐
│ Start with:                             │
│ ○ Weekly Sales Report                   │
│ ○ Customer Analysis                     │
│ ○ Data Exploration                      │
└─────────────────────────────────────────┘
```

Make prompts self-contained—users may not have prior context.

## Testing Prompts

### The "New User" Test

Have someone unfamiliar with your system use the prompt:
- Did they get the expected result?
- Did they understand what was happening?
- Were there any confusing steps?

### The "Edge Case" Test

Try prompts with unusual inputs:
- Empty data sets
- Extremely large result sets
- Missing required resources
- Permission errors mid-workflow

### The "Multi-Server" Test

Test with other MCP servers connected:
- Does the AI still use your tools correctly?
- Are there name collisions in the prompt steps?
- Does the workflow complete reliably?

## Summary

Prompts are your mechanism for:

| Need | Prompt Pattern |
|------|---------------|
| Reliable multi-step workflows | Numbered explicit steps |
| User control over approach | Let users select the prompt |
| Safe operations | Built-in guard rails |
| Consistent output | Defined output format |
| Complex analysis | Multi-turn conversations |
| Conditional behavior | Template variables |

The key insight: **Users invoking prompts are explicitly choosing a workflow.** This is fundamentally different from hoping the AI chooses the right approach. Design prompts that give users the control they need.
