# Resources, Prompts, and Workflows

Tools get most of the attention in MCP discussions, but they're only one-third of the picture. Resources and prompts complete the design space—and prompts, in particular, are the key to giving users control over AI behavior.

## The Control Problem

Recall from Chapter 4: you don't control the AI client's decisions. The AI decides which tools to call, in what order, with what parameters. This creates a fundamental challenge:

**How do you build reliable workflows when you can't control execution?**

The answer lies in understanding what each MCP primitive is designed for:

| Primitive | Purpose | Who Controls |
|-----------|---------|--------------|
| **Tools** | Actions the AI can take | AI decides when/how to use |
| **Resources** | Documents the AI can read | AI decides what to read |
| **Prompts** | Workflows the *user* can invoke | User explicitly selects |

Prompts are the critical insight: they're the only mechanism where the **user** has explicit control, and you, as the MCP developer, have the ability to control the flow.

## Resources: Stable Data for Context

Resources are addressable data that the AI can read. Unlike tools, which perform actions, resources simply provide information. They are the documentation for the AI agents and MCP clients on how to use the tools.

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

## Prompts: User Control and Workflow Execution

Prompts are the most underutilized MCP primitive—and potentially the most powerful for complex workflows.

### The Key Insight

Unlike tools and resources, prompts are **explicitly invoked by users**:

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude Desktop                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  User types: /quarterly-analysis                            │
│              ─────────────────────                          │
│                     │                                       │
│                     ▼                                       │
│  ┌────────────────────────────────────────────┐             │
│  │  Prompt: quarterly-analysis                │             │
│  │  ────────────────────────────────────────  │             │
│  │  Server executes workflow steps            │             │
│  │  and returns results to the AI             │             │
│  └────────────────────────────────────────────┘             │
│                                                             │
│  The AI receives pre-executed context                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**This is the control users have been missing.** Instead of hoping the AI takes the right approach, users explicitly select a workflow.

### The Workflow Spectrum: Soft → Hybrid → Hard

PMCP provides a spectrum of workflow execution models. The guiding principle:

> **Do as much as possible on the server side, and allow the AI to complete the workflow if you can't complete it on the server side.**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Workflow Execution Spectrum                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  SOFT WORKFLOWS              HYBRID WORKFLOWS          HARD WORKFLOWS   │
│  ──────────────              ────────────────          ──────────────   │
│  Text guidance               Server executes some      Server executes  │
│  for AI to follow            AI completes the rest     everything       │
│                                                                         │
│  ┌─────────────┐             ┌─────────────┐          ┌─────────────┐   │
│  │ "Follow     │             │ Server:     │          │ Server:     │   │
│  │  these      │             │   Step 1    │          │   Step 1 ✓  │   │
│  │  steps:     │             │   Step 2    │          │   Step 2 ✓  │   │
│  │  1. ...     │             │ AI:         │          │   Step 3 ✓  │   │
│  │  2. ...     │             │   Step 3    │          │   Step 4 ✓  │   │
│  │  3. ..."    │             │   Step 4    │          │ Return:     │   │
│  └─────────────┘             └─────────────┘          │  Complete   │   │
│                                                       │  results    │   │
│                                                       └─────────────┘   │
│                                                                         │
│  Use when:                   Use when:                 Use when:        │
│  - Complex reasoning         - Some steps need         - All steps are  │
│    required                    LLM judgment              deterministic  │
│  - Context-dependent         - Fuzzy matching          - No reasoning   │
│    decisions                 - User clarification        needed         │
│  - Dynamic exploration         may be needed           - Single result  │
│                                                                         │
│  Examples:                   Examples:                 Examples:        │
│  - Open-ended analysis       - "Add task to project"   - Data pipelines │
│  - Creative tasks              (fuzzy project name)    - Report gen     │
│  - Multi-domain queries      - Search + refine         - CRUD workflows │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  ◄─── Less deterministic           More deterministic ───►              │
│  ◄─── More AI reasoning            Less AI reasoning ───►               │
│  ◄─── Multiple round-trips         Single round-trip ───►               │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why Prefer Hard Workflows?

Hard workflows (server-side execution) provide significant advantages:

| Aspect | Soft Workflow | Hard Workflow |
|--------|---------------|---------------|
| **Round-trips** | 1 per tool call | 1 total |
| **Execution order** | AI decides (unpredictable) | Server enforces (deterministic) |
| **Data binding** | AI must remember | Server manages automatically |
| **Error handling** | AI interprets | Server controls |
| **Testing** | Requires AI | Pure function tests |
| **Latency** | High (multiple LLM calls) | Low (single execution) |

**Best practice**: Start with hard workflows. Fall back to hybrid or soft only when LLM reasoning is genuinely required.

### How MCP Clients Expose Prompts

Different clients expose prompts differently:

**Claude Desktop / Claude Code:**
- Prompts appear as slash commands: `/analyze-schema`, `/generate-report`
- Users see a list of available prompts from connected servers
- Arguments are collected interactively

**VS Code / Cursor:**
- Prompts appear in command palette
- Can be bound to keyboard shortcuts
- Context-aware prompt suggestions

## PMCP SDK: Workflow Types

The PMCP SDK provides two approaches to prompts:

### 1. Text Prompts (Soft Workflows)

For guidance-based workflows where AI follows instructions:

```rust
use pmcp::server::PromptHandler;

Prompt::new("data-exploration")
    .description("Interactive data exploration session")
    .messages(vec![
        PromptMessage::user(
            "Start an interactive data exploration session:\n\n\
            **Initial Setup:**\n\
            1. Read available schemas\n\
            2. List tables and their row counts\n\
            3. Present a summary of available data\n\n\
            **Then wait for my questions...**"
        )
    ])
```

### 2. Sequential Workflows (Hard/Hybrid Workflows)

For server-executed workflows with automatic data binding:

```rust
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use pmcp::server::workflow::dsl::*;

let workflow = SequentialWorkflow::new(
    "quarterly_report",
    "Generate quarterly sales report with analysis"
)
.argument("quarter", "Quarter: Q1, Q2, Q3, Q4", true)
.argument("year", "Year (default: current)", false)

// Step 1: Fetch sales data (server executes)
.step(
    WorkflowStep::new("fetch_sales", ToolHandle::new("sales_query"))
        .arg("quarter", prompt_arg("quarter"))
        .arg("year", prompt_arg("year"))
        .bind("sales_data")  // Output bound for next step
)

// Step 2: Calculate metrics (server executes)
.step(
    WorkflowStep::new("calc_metrics", ToolHandle::new("calculate_metrics"))
        .arg("data", from_step("sales_data"))  // Use previous output
        .bind("metrics")
)

// Step 3: Generate report (server executes)
.step(
    WorkflowStep::new("generate_report", ToolHandle::new("format_report"))
        .arg("sales", from_step("sales_data"))
        .arg("metrics", from_step("metrics"))
        .arg("format", constant(json!("markdown")))
        .bind("report")
);

// Register with server
let server = Server::builder()
    .name("sales-server")
    .version("1.0.0")
    .prompt_workflow(workflow)?
    .build()?;
```

When a user invokes `/quarterly_report Q3 2024`:
1. Server receives `prompts/get` request
2. Server executes all three steps sequentially
3. Server binds outputs between steps automatically
4. Server returns complete conversation trace with results
5. AI receives pre-computed data—no additional tool calls needed

## Combining Primitives

The real power comes from using all three primitives together:

```rust
// RESOURCES: Stable reference data
Resource::new("sales://schema")
Resource::new("sales://regions")
Resource::new("sales://products")

// TOOLS: Actions for direct use and workflow steps
Tool::new("sales_query")       // Query data
Tool::new("sales_aggregate")   // Calculate summaries
Tool::new("sales_export")      // Export results

// PROMPTS: User-controlled workflows

// Soft workflow for exploration
Prompt::new("data-exploration")
    .messages(vec![...])

// Hard workflow for reports
SequentialWorkflow::new("quarterly-analysis")
    .step(WorkflowStep::new(...))
    .step(WorkflowStep::new(...))
```

A user invoking `/quarterly-analysis`:
1. **Workflow** executes all steps server-side
2. **Resources** provide context (schema, regions)
3. **Tools** perform the actual queries
4. Result: Complete report in single round-trip

Without the workflow, the AI might:
- Query random tables
- Miss the year-over-year comparison
- Forget to check all regions
- Present data in an inconsistent format
- Require 6+ round-trips for 3-step workflow

## Summary

| Primitive | Design Question | User Experience |
|-----------|-----------------|-----------------|
| **Tools** | "What actions should be possible?" | AI uses as needed |
| **Resources** | "What context should be available?" | AI reads for understanding |
| **Prompts** | "What workflows should users control?" | User explicitly invokes |

The key insight: **Do as much as possible on the server side.** Use hard workflows by default, falling back to hybrid or soft workflows only when genuine LLM reasoning is required.

| Workflow Type | When to Use |
|---------------|-------------|
| **Hard** | All steps are deterministic, no reasoning needed |
| **Hybrid** | Some steps need LLM judgment (fuzzy matching, clarification) |
| **Soft** | Complex reasoning, exploration, creative tasks |

Next, we'll explore text prompts for guidance-based workflows, then dive deep into the SequentialWorkflow DSL for server-side execution.
