# Resources, Prompts, and Workflows

Tools get most of the attention in MCP discussions, but they're only one-third of the picture. Resources and prompts complete the design space—and prompts, in particular, are the key to giving users control over AI behavior.

## The Control Problem

Recall from Chapter 4: you don't control the AI client's decisions. The AI decides which tools to call, in what order, with what parameters. This creates a fundamental challenge:

**How do you build reliable workflows when you can't control execution?**

The answer lies in understanding what each MCP primitive is designed for:

| Primitive | Purpose | Who Controls |
|-----------|---------|--------------|
| **Tools** | Actions the AI can take | AI decides when/how to use |
| **Resources** | Data the AI can read | AI decides what to read |
| **Prompts** | Workflows the *user* can invoke | User explicitly selects |

Prompts are the critical insight: they're the only mechanism where the **user** has explicit control.

## Resources: Stable Data for Context

Resources are addressable data that the AI can read. Unlike tools, which perform actions, resources simply provide information.

### When to Use Resources

Use resources for data that:
- Has a stable identity (URI)
- Doesn't require computation to retrieve
- Provides context for tool usage
- Shouldn't trigger actions just by being read

```rust
// Database schema - stable reference data
Resource::new("sales://schema/customers")
    .name("Customer Table Schema")
    .description("Column definitions, types, and constraints for the customers table")
    .mime_type("application/json")

// Configuration - current settings
Resource::new("sales://config/regions")
    .name("Sales Regions")
    .description("Active sales regions with territory mappings")
    .mime_type("application/json")

// Templates - reusable patterns
Resource::new("sales://templates/reports")
    .name("Report Templates")
    .description("Available report formats and their parameters")
    .mime_type("application/json")
```

### Resources vs Tools

A common mistake is implementing read operations as tools when they should be resources:

```rust
// WRONG: Read-only data as a tool
Tool::new("get_schema")
    .description("Get the database schema")
// This implies an action, but it's just reading data

// RIGHT: Read-only data as a resource
Resource::new("db://schema")
    .description("Database schema with all tables and columns")
// Clear that this is stable, readable data
```

The AI treats resources differently than tools:
- Resources can be read proactively for context
- Resources don't count as "actions taken"
- Resources are cached by many clients

### Dynamic Resources with Templates

Resources can include URI templates for parameterized access:

```rust
Resource::new("sales://customers/{customer_id}")
    .name("Customer Details")
    .description("Detailed information for a specific customer")

Resource::new("sales://reports/{year}/{quarter}")
    .name("Quarterly Report")
    .description("Sales report for a specific quarter")
```

## Prompts: User Control Mechanism

Prompts are the most underutilized MCP primitive—and potentially the most powerful for complex workflows.

### The Key Insight

Unlike tools and resources, prompts are **explicitly invoked by users**:

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude Desktop                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  User types: /quarterly-analysis                             │
│              ─────────────────────                           │
│                     │                                        │
│                     ▼                                        │
│  ┌────────────────────────────────────────────┐             │
│  │  Prompt: quarterly-analysis                 │             │
│  │  ────────────────────────────────────────  │             │
│  │  "I'll analyze quarterly performance       │             │
│  │   using the following approach:            │             │
│  │                                            │             │
│  │   1. Gather sales data for the quarter    │             │
│  │   2. Compare against previous quarters    │             │
│  │   3. Identify trends and anomalies        │             │
│  │   4. Generate actionable insights         │             │
│  │                                            │             │
│  │   Which quarter would you like to analyze?"│             │
│  └────────────────────────────────────────────┘             │
│                                                              │
│  The AI now follows this structured approach                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**This is the control users have been missing.** Instead of hoping the AI takes the right approach, users explicitly select a workflow.

### How MCP Clients Expose Prompts

Different clients expose prompts differently:

**Claude Desktop / Claude Code:**
- Prompts appear as slash commands: `/analyze-schema`, `/generate-report`
- Users see a list of available prompts from connected servers
- Arguments are collected interactively

**ChatGPT (with MCP plugin):**
- Prompts appear in the conversation starters
- Users can select from available workflows
- Multi-turn prompts guide the conversation

**Cursor / VS Code:**
- Prompts appear in command palette
- Can be bound to keyboard shortcuts
- Context-aware prompt suggestions

### Designing Effective Prompts

Prompts should guide the AI toward a reliable workflow:

```rust
Prompt::new("quarterly-sales-analysis")
    .description("Comprehensive quarterly sales analysis with trends and forecasts")
    .arguments(vec![
        PromptArgument::new("quarter")
            .description("Quarter to analyze: Q1, Q2, Q3, or Q4")
            .required(true),
        PromptArgument::new("year")
            .description("Year (defaults to current year)")
            .required(false),
        PromptArgument::new("compare_previous")
            .description("Include year-over-year comparison")
            .required(false),
    ])
    .messages(vec![
        PromptMessage::user(
            "Analyze sales performance for {{quarter}} {{year}}. \
            \n\nPlease follow these steps:\
            \n1. First, read the sales://schema resource to understand available data\
            \n2. Query total revenue, units sold, and customer count for the quarter\
            \n3. Break down by region and product category\
            \n4. {{#if compare_previous}}Compare against the same quarter last year{{/if}}\
            \n5. Identify the top 3 trends or anomalies\
            \n6. Provide 2-3 actionable recommendations\
            \n\nFormat the output with clear sections and include relevant numbers."
        )
    ])
```

### Prompt Design Patterns

**1. The Structured Workflow**

Guide the AI through a specific sequence:

```rust
Prompt::new("customer-health-check")
    .messages(vec![
        PromptMessage::user(
            "Perform a customer health check for {{customer_id}}:\
            \n\n## Step 1: Gather Context\
            \n- Read customer profile from sales://customers/{{customer_id}}\
            \n- Get recent order history (last 6 months)\
            \n\n## Step 2: Analyze Behavior\
            \n- Calculate order frequency trend\
            \n- Identify any declining metrics\
            \n- Check for support tickets or complaints\
            \n\n## Step 3: Risk Assessment\
            \n- Assign churn risk: Low/Medium/High\
            \n- Justify with specific data points\
            \n\n## Step 4: Recommendations\
            \n- Suggest 2-3 retention actions if risk is Medium or High\
            \n- Otherwise, suggest upsell opportunities"
        )
    ])
```

**2. The Context-Setting Prompt**

Establish context before the user's actual task:

```rust
Prompt::new("sales-analysis-mode")
    .description("Enter sales analysis mode with full context")
    .messages(vec![
        PromptMessage::user(
            "I'm going to analyze sales data. Before I ask my questions:\
            \n\n1. Read the sales://schema resource\
            \n2. Read the sales://config/regions resource\
            \n3. Summarize what data is available and any recent changes\
            \n\nThen wait for my analysis questions."
        )
    ])
```

**3. The Guard Rails Prompt**

Prevent dangerous operations:

```rust
Prompt::new("safe-data-export")
    .description("Export data with compliance checks")
    .messages(vec![
        PromptMessage::user(
            "Help me export {{table}} data. Before exporting:\
            \n\n1. Check if this table contains PII columns\
            \n2. If PII exists, confirm I have a legitimate business need\
            \n3. Offer options to anonymize or redact sensitive columns\
            \n4. Limit export to 10,000 rows unless I explicitly request more\
            \n\nProceed with export only after these checks."
        )
    ])
```

## Combining Primitives

The real power comes from using all three primitives together:

```rust
// RESOURCES: Stable reference data
Resource::new("sales://schema")
Resource::new("sales://regions")
Resource::new("sales://products")

// TOOLS: Actions the AI can take
Tool::new("sales_query")       // Query data
Tool::new("sales_aggregate")   // Calculate summaries
Tool::new("sales_export")      // Export results

// PROMPTS: User-controlled workflows
Prompt::new("quarterly-analysis")    // Structured analysis flow
Prompt::new("data-exploration")      // Guided exploration
Prompt::new("safe-export")           // Guarded export workflow
```

A user invoking `/quarterly-analysis`:
1. **Prompt** guides the AI's approach
2. **Resources** provide context (schema, regions)
3. **Tools** perform the actual queries
4. Result: Predictable, reliable analysis

Without the prompt, the AI might:
- Query random tables
- Miss the year-over-year comparison
- Forget to check all regions
- Present data in an inconsistent format

## Summary

| Primitive | Design Question | User Experience |
|-----------|-----------------|-----------------|
| **Tools** | "What actions should be possible?" | AI uses as needed |
| **Resources** | "What context should be available?" | AI reads for understanding |
| **Prompts** | "What workflows should users control?" | User explicitly invokes |

The key insight: **Prompts are your primary mechanism for reliable, user-controlled workflows.** Don't just expose tools and hope the AI uses them correctly—design prompts that guide the AI toward the outcomes your users need.

Next, we'll dive deeper into when to use resources vs tools, and then explore prompts as workflow templates in detail.
