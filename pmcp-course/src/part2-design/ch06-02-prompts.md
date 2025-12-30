# Soft Workflows: Text Prompts for AI Guidance

When hard workflows aren't possible—when steps require LLM reasoning, context-dependent decisions, or creative interpretation—text prompts provide structured guidance for AI execution.

## When to Use Soft Workflows

Remember the guiding principle: **Do as much as possible on the server side.** Use soft workflows only when:

| Scenario | Why Soft Workflow |
|----------|-------------------|
| **Complex reasoning required** | AI must interpret, analyze, or synthesize |
| **Context-dependent decisions** | Right choice depends on conversation history |
| **Dynamic exploration** | AI discovers what to do based on findings |
| **Creative or open-ended tasks** | Multiple valid approaches exist |
| **Multi-domain queries** | AI must coordinate across many servers |

If all steps are deterministic, use a [hard workflow](./ch06-03-workflows.md) instead.

## The Soft Workflow Tradeoff

```
┌────────────────────────────────────────────────────────────────────┐
│                    Soft Workflow Execution                         │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  Client                          Server                            │
│    │                               │                               │
│    │──── prompts/get ─────────────►│                               │
│    │◄─── text guidance ────────────│                               │
│    │                               │                               │
│    │  AI reads guidance...         │                               │
│    │  AI decides to call tool 1    │                               │
│    │                               │                               │
│    │──── tools/call (tool 1) ─────►│                               │
│    │◄─── result 1 ─────────────────│                               │
│    │                               │                               │
│    │  AI processes result...       │                               │
│    │  AI decides to call tool 2    │                               │
│    │                               │                               │
│    │──── tools/call (tool 2) ─────►│                               │
│    │◄─── result 2 ─────────────────│                               │
│    │                               │                               │
│    │  ... more round trips ...     │                               │
│    │                               │                               │
│    │  AI synthesizes final answer  │                               │
│    ▼                               ▼                               │
│                                                                    │
│  Total: 1 + N round trips (where N = number of tool calls)         │
└────────────────────────────────────────────────────────────────────┘
```

**Trade-off**: More flexibility, but more latency and less predictable execution.

## Text Prompt Design Principles

### 1. Be Explicit About Steps

The AI follows instructions better when steps are clearly numbered:

```rust
Prompt::new("database-audit")
    .description("Comprehensive database security audit")
    .messages(vec![
        PromptMessage::user(
            "Perform a security audit of the database:\n\n\
            **Step 1: Schema Analysis**\n\
            - Read db://schema to understand table structure\n\
            - Identify tables containing PII or sensitive data\n\n\
            **Step 2: Access Review**\n\
            - List all users with write permissions\n\
            - Flag any overly broad permission grants\n\n\
            **Step 3: Data Exposure Check**\n\
            - Check for unencrypted sensitive columns\n\
            - Verify no credentials stored in plain text\n\n\
            **Step 4: Report**\n\
            - Summarize findings with severity ratings\n\
            - Provide specific remediation recommendations\n\n\
            Begin with Step 1."
        )
    ])
```

### 2. Reference Specific Resources and Tools

Don't leave the AI guessing which tools to use:

```rust
Prompt::new("customer-360-view")
    .messages(vec![
        PromptMessage::user(
            "Create a 360-degree view of customer {{customer_id}}:\n\n\
            1. **Profile**: Read resource `customers://{{customer_id}}/profile`\n\
            2. **Orders**: Use `sales_query` to get order history\n\
            3. **Support**: Use `tickets_query` to get support interactions\n\
            4. **Payments**: Use `billing_query` to get payment history\n\n\
            Synthesize into a comprehensive customer summary."
        )
    ])
```

### 3. Define Output Format

Specify how results should be presented:

```rust
Prompt::new("weekly-metrics-report")
    .messages(vec![
        PromptMessage::user(
            "Generate the weekly metrics report:\n\n\
            ## Data to Gather\n\
            - Revenue by region (use sales_aggregate)\n\
            - New customers (use customers_query)\n\
            - Support tickets (use tickets_summary)\n\n\
            ## Output Format\n\
            ```\n\
            # Weekly Metrics: {{week_start}} - {{week_end}}\n\n\
            ## Revenue\n\
            | Region | This Week | Last Week | Change |\n\
            |--------|-----------|-----------|--------|\n\
            | ...    | ...       | ...       | ...    |\n\n\
            ## Key Insights\n\
            1. [Insight 1]\n\
            2. [Insight 2]\n\
            ```"
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
            "Help me modify data in {{table}}:\n\n\
            **Safety Protocol:**\n\
            1. First, show me the current state of affected records\n\
            2. Explain exactly what changes will be made\n\
            3. Ask for my explicit confirmation before proceeding\n\
            4. After modification, show the before/after comparison\n\n\
            **Constraints:**\n\
            - Maximum 100 records per operation\n\
            - No DELETE operations without WHERE clause\n\
            - All changes must be logged\n\n\
            What modification do you need?"
        )
    ])
```

## Soft Workflow Patterns

### Pattern 1: The Context-Setting Prompt

Establish context before the user's actual task:

```rust
Prompt::new("sales-analysis-mode")
    .description("Enter sales analysis mode with full context")
    .messages(vec![
        PromptMessage::user(
            "I'm going to analyze sales data. Before I ask my questions:\n\n\
            1. Read the sales://schema resource\n\
            2. Read the sales://config/regions resource\n\
            3. Summarize what data is available and any recent changes\n\n\
            Then wait for my analysis questions."
        )
    ])
```

**When to use**: User will ask multiple follow-up questions; context needs to be established first.

### Pattern 2: The Exploration Prompt

Guide AI through discovery:

```rust
Prompt::new("data-exploration")
    .description("Interactive data exploration session")
    .messages(vec![
        PromptMessage::user(
            "Start an interactive data exploration session:\n\n\
            **Initial Setup:**\n\
            1. Read available schemas\n\
            2. List tables and their row counts\n\
            3. Present a summary of available data\n\n\
            **Then wait for my questions. For each question:**\n\
            - If I ask about data: query and visualize\n\
            - If I ask about relationships: show joins and keys\n\
            - If I ask for export: use safe_export with confirmation\n\n\
            **Session rules:**\n\
            - Keep queries under 10,000 rows\n\
            - Warn before expensive operations\n\
            - Maintain context across questions\n\n\
            Begin setup."
        )
    ])
```

**When to use**: Open-ended exploration where the path isn't known in advance.

### Pattern 3: The Investigation Prompt

Drill-down analysis with dynamic branching:

```rust
Prompt::new("investigate-anomaly")
    .arguments(vec![
        PromptArgument::new("severity")
            .description("Alert severity: low, medium, high, critical"),
        PromptArgument::new("metric")
            .description("The metric that triggered the alert"),
    ])
    .messages(vec![
        PromptMessage::user(
            "Investigate the {{severity}} severity anomaly in {{metric}}:\n\n\
            {{#if severity == 'critical'}}\n\
            **CRITICAL ALERT PROTOCOL:**\n\
            1. Immediately gather last 24 hours of data\n\
            2. Compare against last 7 days baseline\n\
            3. Identify correlated metrics\n\
            4. Check for system events at anomaly time\n\
            5. Prepare incident summary for escalation\n\
            {{else if severity == 'high'}}\n\
            **HIGH ALERT INVESTIGATION:**\n\
            1. Gather last 48 hours of data\n\
            2. Identify pattern or one-time spike\n\
            3. Check for known causes\n\
            4. Recommend monitoring or action\n\
            {{else}}\n\
            **STANDARD INVESTIGATION:**\n\
            1. Review metric trend for past week\n\
            2. Note if this is recurring\n\
            3. Log finding for pattern analysis\n\
            {{/if}}"
        )
    ])
```

**When to use**: Response should vary based on parameters; complex conditional logic.

### Pattern 4: Chained Prompts

Design prompts that build on each other:

```rust
// First prompt: Discovery
Prompt::new("discover-opportunities")
    .description("Find potential opportunities in sales data")
    .messages(vec![
        PromptMessage::user(
            "Analyze sales data to identify opportunities:\n\n\
            1. Find underperforming products in growing categories\n\
            2. Identify customers with declining purchase frequency\n\
            3. Spot regions with untapped potential\n\n\
            List findings with IDs for follow-up analysis.\n\
            User can then run /deep-dive on any finding."
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
            "Perform detailed analysis on finding {{finding_id}}:\n\n\
            1. Gather all related data\n\
            2. Analyze root causes\n\
            3. Model potential impact of intervention\n\
            4. Provide specific, actionable recommendations\n\
            5. Estimate effort and expected return"
        )
    ])
```

**When to use**: User workflow naturally has distinct phases; each phase produces different outputs.

## Converting Soft to Hard Workflows

As you gain experience with a soft workflow, look for opportunities to harden it:

| Soft Pattern | Can It Be Hardened? |
|--------------|---------------------|
| Fixed sequence of tool calls | **Yes** → Use `SequentialWorkflow` |
| Deterministic data gathering | **Yes** → Use server-side steps |
| Fuzzy matching user input | **Hybrid** → Server gathers, AI matches |
| Dynamic branching based on results | **Maybe** → Complex, evaluate case-by-case |
| Creative interpretation | **No** → Keep as soft workflow |
| Multi-domain coordination | **No** → AI must reason across servers |

### Example: Hardening a Report Workflow

**Before (Soft):**
```rust
Prompt::new("weekly-report")
    .messages(vec![
        PromptMessage::user(
            "Generate weekly sales report:\n\
            1. Query revenue by region\n\
            2. Calculate week-over-week change\n\
            3. Format as markdown table"
        )
    ])
```

**After (Hard):**
```rust
SequentialWorkflow::new("weekly_report", "Generate weekly sales report")
    .argument("week", "Week number (1-52)", true)
    .step(
        WorkflowStep::new("current", ToolHandle::new("sales_query"))
            .arg("week", prompt_arg("week"))
            .bind("current_data")
    )
    .step(
        WorkflowStep::new("previous", ToolHandle::new("sales_query"))
            .arg("week", /* week - 1 calculation */)
            .bind("previous_data")
    )
    .step(
        WorkflowStep::new("format", ToolHandle::new("format_report"))
            .arg("current", from_step("current_data"))
            .arg("previous", from_step("previous_data"))
            .bind("report")
    )
```

The hard workflow executes in a single round-trip with deterministic results.

## Testing Soft Workflows

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

Soft workflows are appropriate when:

| Scenario | Use Soft Workflow |
|----------|-------------------|
| AI reasoning required | Text prompts guide interpretation |
| Exploration/discovery | AI determines path based on findings |
| Complex conditionals | AI evaluates and branches |
| Multi-server coordination | AI reasons across domains |
| Creative tasks | Multiple valid approaches |

Design effective soft workflows by:
1. **Being explicit** - Numbered steps, specific tools, clear output formats
2. **Including guard rails** - Safety checks, constraints, confirmations
3. **Setting context** - Read resources before acting
4. **Enabling follow-up** - Chained prompts for multi-phase workflows

Remember: **Start with hard workflows.** Convert to soft workflows only when genuine LLM reasoning is required. The next chapter covers `SequentialWorkflow` for server-side execution.
