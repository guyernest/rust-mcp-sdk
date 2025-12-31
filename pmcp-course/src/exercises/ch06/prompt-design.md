::: exercise
id: ch06-01-prompt-design
difficulty: intermediate
time: 25 minutes
:::

Your company has an MCP server with great tools, but users complain that the AI
"doesn't do what they expect." After investigation, you realize the problem:
users ask vague questions and the AI picks arbitrary approaches.

Your task is to design prompts that give users control over AI behavior by
defining explicit workflows.

::: objectives
thinking:
  - Understand prompts as user-controlled workflows, not just templates
  - Design multi-step prompts with explicit tool references
  - Include guard rails for dangerous operations
  - Create prompts that work across different MCP clients
doing:
  - Write a structured analysis prompt with numbered steps
  - Design a safe data modification prompt with confirmation
  - Create a context-setting prompt for exploratory work
:::

::: discussion
- What's the difference between a user asking "analyze sales" vs invoking /sales-analysis?
- Why should prompts reference specific tools by name?
- How do Claude Desktop, ChatGPT, and VS Code expose prompts differently?
:::

::: starter file="prompts.md"
```markdown
# Prompt Design Exercise

You have an MCP server with these tools and resources:

## Available Tools
- `sales_query`: Query sales data with filters
- `sales_aggregate`: Calculate totals, averages, trends
- `customer_get`: Get customer details by ID
- `customer_health`: Calculate customer health score
- `report_generate`: Generate formatted reports
- `data_export`: Export data to CSV/JSON

## Available Resources
- `sales://schema`: Sales database schema
- `sales://regions`: List of sales regions
- `config://limits`: API rate limits and quotas

---

## Task 1: Analysis Prompt

Design a prompt for `/quarterly-analysis` that:
- Gathers Q1-Q4 sales data systematically
- Compares against previous year
- Identifies top 3 trends
- Produces a formatted report

Current (vague) version:
```
Analyze quarterly sales performance.
```

Your improved version:
```
TODO: Write a structured prompt with:
- Numbered steps
- Specific tool references
- Expected output format
```

---

## Task 2: Safe Modification Prompt

Design a prompt for `/bulk-update` that:
- Shows affected records BEFORE making changes
- Requires explicit confirmation
- Limits batch size for safety
- Provides rollback information

Current (dangerous) version:
```
Update customer records as requested.
```

Your improved version:
```
TODO: Write a prompt with guard rails
```

---

## Task 3: Context-Setting Prompt

Design a prompt for `/sales-mode` that:
- Reads relevant resources for context
- Summarizes available data
- Waits for user's actual questions
- Sets expectations for the session

Current (none exists):
```
(Users just start asking questions cold)
```

Your improved version:
```
TODO: Write a context-setting prompt
```

---

## Task 4: Error Recovery

Design how prompts should handle tool failures:
- What if sales_query returns an error?
- What if the user doesn't have permission?
- What if rate limits are hit?

Add error handling guidance to your Task 1 prompt.
```
:::

::: hint level=1 title="Structured prompt template"
Follow this template for analysis prompts:
```
Perform [analysis name] for [parameters]:

**Step 1: Gather Context**
- Read [resource] to understand [what]
- Note [what to look for]

**Step 2: Collect Data**
- Use [tool] with [parameters]
- Use [tool] with [parameters]

**Step 3: Analyze**
- Calculate [metrics]
- Compare [comparisons]
- Identify [patterns]

**Step 4: Report**
Format output as:
[template]
```
:::

::: hint level=2 title="Guard rails pattern"
For dangerous operations, include safety checks:
```
Before making any changes:

1. **Preview Phase**
   - Query affected records using [tool]
   - Display: count, sample records, potential impact
   - If more than [N] records, warn and ask to proceed

2. **Confirmation Phase**
   - Summarize exactly what will change
   - Ask for explicit "yes" to proceed
   - Any other response = abort

3. **Execution Phase**
   - Process in batches of [N]
   - Log each batch result
   - Stop on first error

4. **Verification Phase**
   - Query results to confirm changes
   - Report success/failure summary
```
:::

::: hint level=3 title="Context-setting pattern"
For exploration prompts:
```
Initialize [domain] exploration session:

**Setup:**
1. Read [resource1] - note [what to learn]
2. Read [resource2] - note [what to learn]
3. Summarize available data and capabilities

**Present to user:**
- What data is available
- What operations are possible
- Any current limitations (rate limits, permissions)

**Then wait for questions. For each question:**
- If asking about data: use [query tool]
- If asking about trends: use [aggregate tool]
- If asking for export: use [export tool] with confirmation

**Session rules:**
- Limit queries to [N] rows by default
- Warn before expensive operations
- Maintain context across questions
```
:::

::: solution
```markdown
# Prompt Design Solutions

## Task 1: Quarterly Analysis Prompt

```rust
Prompt::new("quarterly-analysis")
    .description("Comprehensive quarterly sales analysis with YoY comparison")
    .arguments(vec![
        PromptArgument::new("quarter")
            .description("Quarter to analyze: Q1, Q2, Q3, or Q4")
            .required(true),
        PromptArgument::new("year")
            .description("Year (defaults to current)")
            .required(false),
    ])
    .messages(vec![
        PromptMessage::user(r#"
Perform quarterly sales analysis for {{quarter}} {{year}}:

**Step 1: Gather Context**
- Read `sales://schema` to understand available data fields
- Read `sales://regions` to get the complete region list
- Note any schema changes that might affect comparisons

**Step 2: Collect Current Quarter Data**
- Use `sales_query` with date_range for {{quarter}} {{year}}
- Use `sales_aggregate` to calculate:
  - Total revenue
  - Units sold
  - Average order value
  - Customer count
- Break down by region using `sales_aggregate` with group_by="region"

**Step 3: Collect Comparison Data**
- Use `sales_query` with date_range for {{quarter}} of previous year
- Use `sales_aggregate` for same metrics
- Calculate year-over-year changes for each metric

**Step 4: Identify Trends**
- Compare regional performance: which regions grew/declined?
- Identify top 3 trends or anomalies
- Note any concerning patterns

**Step 5: Generate Report**
Use `report_generate` with this structure:

```
# Quarterly Sales Analysis: {{quarter}} {{year}}

## Executive Summary
- Total Revenue: $X (+/-Y% YoY)
- Key Insight: [one sentence]

## Performance by Region
| Region | Revenue | YoY Change | Units |
|--------|---------|------------|-------|
| ...    | ...     | ...        | ...   |

## Top 3 Trends
1. [Trend with supporting data]
2. [Trend with supporting data]
3. [Trend with supporting data]

## Recommendations
1. [Actionable recommendation]
2. [Actionable recommendation]
```

**Error Handling:**
- If `sales_query` fails with RATE_LIMITED: wait and retry
- If data is missing for comparison period: note "No YoY data available"
- If any tool fails: report which step failed and what data is missing
"#)
    ])
```

---

## Task 2: Safe Bulk Update Prompt

```rust
Prompt::new("bulk-update")
    .description("Safely update multiple customer records with preview and confirmation")
    .arguments(vec![
        PromptArgument::new("update_type")
            .description("What to update: status, segment, or contact_info"),
    ])
    .messages(vec![
        PromptMessage::user(r#"
Help me update customer records. This is a SENSITIVE operation.

**Safety Protocol - Follow Exactly:**

## Phase 1: Understand the Request
- Ask what records should be updated (filter criteria)
- Ask what the new value should be
- Confirm the update_type matches: {{update_type}}

## Phase 2: Preview (REQUIRED)
- Use `sales_query` to find matching records
- Display:
  - Total count of affected records
  - Sample of first 5 records with current values
  - If >100 records: **STOP** and ask user to narrow criteria

## Phase 3: Confirmation (REQUIRED)
Present this summary:
```
⚠️ BULK UPDATE PREVIEW

Records to update: [count]
Field to change: {{update_type}}
New value: [value]

Sample of changes:
| Customer ID | Current Value | New Value |
| ...         | ...           | ...       |

Type 'yes' to proceed, anything else to cancel.
```

**Wait for explicit 'yes' response. Any other response = ABORT.**

## Phase 4: Execution (only after 'yes')
- Process in batches of 50 records
- After each batch, report: "Updated X of Y records..."
- If any error occurs: STOP and report what succeeded/failed

## Phase 5: Verification
- Query updated records to confirm changes
- Report final summary:
  - Records successfully updated
  - Any failures
  - Rollback command if needed: `bulk-update --rollback [batch_id]`
"#)
    ])
```

---

## Task 3: Context-Setting Prompt

```rust
Prompt::new("sales-mode")
    .description("Enter sales data exploration mode with full context")
    .messages(vec![
        PromptMessage::user(r#"
Initialize a sales data exploration session.

**Setup Phase:**

1. Read `sales://schema`
   - List available tables and key fields
   - Note any date ranges or limitations

2. Read `sales://regions`
   - List all regions for reference
   - Note which have data

3. Read `config://limits`
   - Note current rate limits
   - Check query quotas remaining

**Present Session Overview:**
```
📊 Sales Data Session Ready

Available Data:
- [List tables and date ranges]

Regions: [List regions]

Rate Limits: [X queries remaining this hour]

I can help you:
- Query specific records (use natural filters)
- Calculate aggregates (totals, trends, comparisons)
- Generate reports (formatted summaries)
- Export data (CSV or JSON)

What would you like to explore?
```

**Session Rules:**

For data questions:
- Use `sales_query` with reasonable LIMIT (default 100)
- Show result count and sample if large

For trend/aggregate questions:
- Use `sales_aggregate` instead of computing manually
- Explain what calculations were performed

For exports:
- Confirm before large exports (>1000 records)
- Use `data_export` and provide download info

For permission errors:
- Explain what's not accessible
- Suggest alternatives if possible

Maintain context across questions - reference previous results when relevant.
"#)
    ])
```

---

## Key Design Principles Applied

| Principle | How Applied |
|-----------|-------------|
| **Explicit steps** | Numbered phases with clear actions |
| **Tool references** | Named tools with parameters |
| **Guard rails** | Preview, confirmation, batch limits |
| **Output format** | Templates for consistent reporting |
| **Error handling** | Specific guidance for failure cases |
| **User control** | Confirmation required for dangerous ops |
```
:::

::: tests mode=local
```rust
#[cfg(test)]
mod tests {
    // Conceptual tests for prompt design

    #[test]
    fn prompt_has_numbered_steps() {
        let prompt = include_str!("quarterly_analysis_prompt.txt");
        assert!(prompt.contains("Step 1"));
        assert!(prompt.contains("Step 2"));
    }

    #[test]
    fn prompt_references_specific_tools() {
        let prompt = include_str!("quarterly_analysis_prompt.txt");
        assert!(prompt.contains("sales_query"));
        assert!(prompt.contains("sales_aggregate"));
    }

    #[test]
    fn dangerous_prompt_requires_confirmation() {
        let prompt = include_str!("bulk_update_prompt.txt");
        assert!(prompt.contains("confirmation") || prompt.contains("Confirmation"));
        assert!(prompt.contains("yes"));
    }

    #[test]
    fn prompt_includes_error_handling() {
        let prompt = include_str!("quarterly_analysis_prompt.txt");
        assert!(prompt.contains("Error") || prompt.contains("fail"));
    }
}
```
:::

::: reflection
- How would you test that a prompt produces reliable results?
- Should prompts be version-controlled? How would you update them?
- What happens when tools change but prompts reference old names?
- How do you balance prescriptive steps vs. AI flexibility?
:::

## Related Examples

For more prompt and resource patterns, explore these SDK examples:

- **[06_server_prompts.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/06_server_prompts.rs)** - Server with prompts including code review prompt
- **[17_completable_prompts.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/17_completable_prompts.rs)** - Prompts with auto-completion for arguments
- **[04_server_resources.rs](https://github.com/paiml/rust-mcp-sdk/blob/main/examples/04_server_resources.rs)** - Resources to pair with prompts

Run locally with:
```bash
cargo run --example 06_server_prompts
cargo run --example 17_completable_prompts
```
